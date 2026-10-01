//! The PostHog counts in the operator API: the aggregate stages (3–4) the funnel carries
//! beside its personal ones, and `GET /api/v1/experiments`. Aggregates only, and the line
//! between them and the leads kept (spec §10.1): nothing here divides a lead by a visit.

use std::collections::BTreeMap;

use axum::{
	Json,
	extract::{Query, State, rejection::QueryRejection},
};
use jiff::Timestamp;
use panel::{
	Panel,
	counts::{ExperimentView, SiteSlice},
};
use panel_core::{
	experiment::{Arm, Comparison, MIN_EXPOSURES, Z95, compare},
	funnel::{MIN_SAMPLE, Share},
	ids::BrandId,
	metrics::{IntentChannel, Tally},
};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::api::{ApiError, ApiResult, ShareDto, window};

/// Where the aggregate stages come from, and how fresh they are.
pub fn source_body(imported_at: Option<Timestamp>) -> Value {
	json!({ "source": "posthog", "kind": "aggregate", "imported_at": imported_at.map(|t| t.to_string()) })
}

fn channels(counts: &BTreeMap<IntentChannel, u64>) -> Value {
	Value::Object(
		IntentChannel::ALL
			.into_iter()
			.map(|c| (c.as_str().to_owned(), json!(counts.get(&c).copied().unwrap_or(0))))
			.collect(),
	)
}

/// A slice's aggregate stages: `site.visit` (by source) and `contact.intent` (by channel),
/// each in all and per day — the days with a count only.
pub fn aggregate_body(s: SiteSlice) -> Value {
	let mut intents = BTreeMap::new();
	let (mut visits_days, mut intent_days) = (Vec::new(), Vec::new());
	for (day, d) in &s.days {
		for (c, n) in &d.intents {
			*intents.entry(*c).or_insert(0) += n;
		}
		if d.visits > 0 {
			visits_days.push(json!({"day": day.to_string(), "n": d.visits}));
		}
		let n: u64 = d.intents.values().sum();
		if n > 0 {
			intent_days.push(json!({"day": day.to_string(), "n": n, "by_channel": channels(&d.intents)}));
		}
	}
	json!({
		"stages": [
			{
				"stage": "site.visit",
				"total": s.sources.values().sum::<u64>(),
				"by_source": s.sources,
				"days": visits_days,
			},
			{
				"stage": "contact.intent",
				"total": intents.values().sum::<u64>(),
				"by_channel": channels(&intents),
				"days": intent_days,
			},
		]
	})
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExperimentsQuery {
	from: Option<String>,
	to: Option<String>,
	brand: Option<String>,
}

fn comparison(c: Comparison) -> Value {
	json!({
		"difference": c.difference.map(|d| json!({"estimate": d.estimate, "low": d.low, "high": d.high, "decimals": d.decimals})),
		"insufficient": c.insufficient.is_some(),
		"reason": c.insufficient.map(|r| r.as_str()),
	})
}

fn rate(successes: u64, exposures: u64) -> ShareDto {
	Share::new(successes.min(exposures), exposures).into()
}

fn experiment_body(e: ExperimentView) -> Value {
	let control = e.variants.iter().find(|v| v.variant == e.control).map(|v| v.tally).unwrap_or_default();
	let metrics = |t: &Tally| [("lead", t.leads), ("contact", t.contacts())];
	let variants: Vec<Value> = e
		.variants
		.iter()
		.map(|v| {
			let t = &v.tally;
			let is_control = v.variant == e.control;
			let rates: serde_json::Map<String, Value> = metrics(t).into_iter().map(|(k, n)| (k.to_owned(), json!(rate(n, t.exposures)))).collect();
			let vs_control = (!is_control).then(|| {
				let pairs = metrics(&control).into_iter().zip(metrics(t));
				Value::Object(
					pairs
						.map(|((k, c), (_, n))| (k.to_owned(), comparison(compare(Arm::new(c, control.exposures), Arm::new(n, t.exposures)))))
						.collect(),
				)
			});
			json!({
				"variant": v.variant,
				"control": is_control,
				"exposures": t.exposures,
				"leads": t.leads,
				"intents": {"phone": t.phone, "whatsapp": t.whatsapp, "form_open": t.form_open, "booking": t.booking},
				"rates": rates,
				"vs_control": vs_control,
			})
		})
		.collect();
	json!({
		"brand": e.brand,
		"experiment": e.experiment,
		"first_day": e.first_day.to_string(),
		"last_day": e.last_day.to_string(),
		"control": e.control,
		"variants": variants,
	})
}

/// `GET /api/v1/experiments?brand&from&to`: per experiment and variant, what PostHog counted
/// over the window, the rates, and each variant against the control — the difference and its
/// 95 % interval, `insufficient` while an arm is small or the interval holds zero. No winner.
pub async fn experiments(State(panel): State<Panel>, q: Result<Query<ExperimentsQuery>, QueryRejection>) -> ApiResult<Json<Value>> {
	let Query(q) = q.map_err(|e| ApiError::BadRequest(e.body_text()))?;
	let (from, to) = window(q.from.as_deref(), q.to.as_deref())?;
	let brand = q.brand.as_deref().map(BrandId::parse).transpose()?;
	let found = panel.experiments(from, to, brand.as_ref()).await?;
	Ok(Json(json!({
		"from": from.to_string(),
		"to": to.to_string(),
		"brand": brand.as_ref().map(BrandId::as_str),
		"min_sample": MIN_SAMPLE,
		"min_exposures": MIN_EXPOSURES,
		"confidence": 0.95,
		"z": Z95,
		"interval": "newcombe_hybrid_score",
		"source": source_body(panel.posthog_imported_at().await?),
		"experiments": found.into_iter().map(experiment_body).collect::<Vec<_>>(),
	})))
}
