//! A brand's experiments as configuration (`panel_core::experiment`): what the screens list,
//! what an admin changes (journaled as `experiment.configured`, through the registry like any
//! operator action), and what a landing is answered.
//!
//! The statistics are PostHog's: each experiment carries a link to its funnel there
//! ([`posthog_url`]) when the project is known.

use std::collections::BTreeMap;

use eyre::WrapErr;
use jiff::Timestamp;
use panel_contracts::SCHEMA;
use panel_core::{
	Invalid,
	experiment::{Field, Patch, State},
	ids::BrandId,
};
use serde_json::{Value, json};

use crate::{Panel, operator::Actor, store::experiments as stored};

/// The PostHog project the landings send to, for the links to its insights.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PosthogProject {
	/// The app's origin, e.g. `https://us.posthog.com` (not the capture host).
	pub app_host: String,
	pub project_id: String,
}

/// One experiment as the screens show it.
#[derive(Clone, Debug, PartialEq)]
pub struct ExperimentView {
	pub brand: BrandId,
	pub state: State,
	/// Its funnel in PostHog; `None` without a project configured.
	pub posthog_url: Option<String>,
}

/// Why an admin's change was not made.
#[derive(Debug, thiserror::Error)]
pub enum ExperimentError {
	/// No such experiment of the brand, or one its landing no longer declares.
	#[error("not found")]
	NotFound,
	#[error("{0}")]
	Invalid(Invalid),
	#[error(transparent)]
	Internal(#[from] eyre::Report),
}

/// The fields of an override a landing takes, as it is sent them.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LiveOverride {
	pub enabled: Option<bool>,
	pub weights: Option<Vec<f64>>,
	pub holdout: Option<f64>,
}

/// `{app_host}/project/{id}/insights/new#q=<the query>`: a funnel `experiment_exposed` →
/// `experiment_lead` of this experiment and brand, forced (QA) visits left out, broken down by
/// variant, since the weights last changed or the experiment was first declared.
pub fn posthog_url(project: &PosthogProject, brand: &BrandId, state: &State) -> String {
	let from = state.weights_changed_at.unwrap_or(state.first_declared_at);
	let event = |name: &str| json!({"kind": "EventsNode", "event": name, "name": name});
	let prop = |key: &str, value: &str, operator: &str| json!({"key": key, "value": [value], "operator": operator, "type": "event"});
	let query = json!({
		"kind": "InsightVizNode",
		"source": {
			"kind": "FunnelsQuery",
			"series": [event("experiment_exposed"), event("experiment_lead")],
			"properties": {"type": "AND", "values": [{"type": "AND", "values": [
				prop("experiment", &state.declared.key, "exact"),
				prop("brand_id", brand.as_str(), "exact"),
				prop("forced", "true", "is_not"),
			]}]},
			"breakdownFilter": {"breakdown": "variant", "breakdown_type": "event"},
			"dateRange": {"date_from": from.to_string()},
			"funnelsFilter": {"funnelVizType": "steps"},
		},
	});
	format!(
		"{}/project/{}/insights/new#q={}",
		project.app_host.trim_end_matches('/'),
		project.project_id,
		uri_component(&query.to_string())
	)
}

/// JavaScript's `encodeURIComponent`.
fn uri_component(s: &str) -> String {
	let mut out = String::with_capacity(s.len() * 3);
	for b in s.bytes() {
		if b.is_ascii_alphanumeric() || b"-_.!~*'()".contains(&b) {
			out.push(char::from(b));
		} else {
			out.push_str(&format!("%{b:02X}"));
		}
	}
	out
}

impl Panel {
	fn view(&self, brand: BrandId, state: State) -> ExperimentView {
		let posthog_url = self.posthog.as_ref().map(|p| posthog_url(p, &brand, &state));
		ExperimentView { brand, state, posthog_url }
	}

	/// Every experiment of one brand, or of all, by brand then key; retired ones included.
	pub async fn experiments(&self, brand: Option<&BrandId>) -> eyre::Result<Vec<ExperimentView>> {
		let mut conn = self.store.pool().acquire().await.wrap_err("a connection")?;
		Ok(stored::list(&mut conn, brand).await?.into_iter().map(|(b, s)| self.view(b, s)).collect())
	}

	async fn experiment(&self, brand: &BrandId, key: &str) -> eyre::Result<Option<State>> {
		let mut conn = self.store.pool().acquire().await.wrap_err("a connection")?;
		Ok(stored::list(&mut conn, Some(brand)).await?.into_iter().map(|(_, s)| s).find(|s| s.declared.key == key))
	}

	/// What a brand's landings lay over their code: per current experiment, the override fields
	/// that fit its declaration. Nothing for a brand the panel does not know.
	pub async fn live_experiments(&self, brand: &BrandId) -> eyre::Result<BTreeMap<String, LiveOverride>> {
		let mut conn = self.store.pool().acquire().await.wrap_err("a connection")?;
		let mut out = BTreeMap::new();
		for (_, s) in stored::list(&mut conn, Some(brand)).await? {
			if s.retired {
				continue;
			}
			let (enabled, weights, holdout) = s.valid_override();
			let o = LiveOverride {
				enabled,
				weights: weights.map(<[f64]>::to_vec),
				holdout,
			};
			if o != LiveOverride::default() {
				out.insert(s.declared.key.clone(), o);
			}
		}
		Ok(out)
	}

	/// An admin's change to one experiment: journaled as `experiment.configured` when it changes
	/// what is set, then answered as the screens show it.
	pub async fn configure_experiment(&self, by: Actor, brand: &BrandId, patch: Patch, now: Timestamp) -> Result<ExperimentView, ExperimentError> {
		let current = self.experiment(brand, &patch.key).await?.filter(|s| !s.retired).ok_or(ExperimentError::NotFound)?;
		current.check_patch(&patch).map_err(ExperimentError::Invalid)?;
		if !current.changes(&patch) {
			return Ok(self.view(brand.clone(), current));
		}
		let mut properties = json!({"key": patch.key});
		let mut reset = Vec::new();
		match &patch.enabled {
			Field::Keep => {}
			Field::Set(v) => properties["enabled"] = json!(v),
			Field::Reset => reset.push("enabled"),
		}
		match &patch.weights {
			Field::Keep => {}
			Field::Set(v) => properties["weights"] = json!(v),
			Field::Reset => reset.push("weights"),
		}
		match &patch.holdout {
			Field::Keep => {}
			Field::Set(v) => properties["holdout"] = json!(v),
			Field::Reset => reset.push("holdout"),
		}
		if !reset.is_empty() {
			properties["reset"] = json!(reset);
		}
		let raw = json!({
			"id": crate::operator::new_event_id(now).to_string(),
			"schema": SCHEMA,
			"type": "experiment.configured",
			"typeVersion": 1,
			"occurredAt": now.to_string(),
			"source": {"kind": "panel", "id": by.0.to_string()},
			"subject": {"brandId": brand.as_str()},
			"properties": properties,
		});
		match self.write_own(raw, now).await?.map_err(ExperimentError::Invalid)?.0 {
			crate::Outcome::Accepted { .. } => {}
			crate::Outcome::Duplicate => return Err(ExperimentError::Internal(eyre::eyre!("a fresh event id was taken"))),
			crate::Outcome::Rejected(e) => return Err(ExperimentError::Invalid(e)),
		}
		tracing::info!(user_id = %by.0, %brand, key = patch.key, "experiment configured");
		let state = self.experiment(brand, &patch.key).await?.ok_or_else(|| eyre::eyre!("an experiment configured and gone"))?;
		Ok(self.view(brand.clone(), state))
	}
}

/// A JSON object of the override fields that are set, as a landing reads them.
pub fn live_json(o: &LiveOverride) -> Value {
	let mut v = json!({});
	if let Some(e) = o.enabled {
		v["enabled"] = json!(e);
	}
	if let Some(w) = &o.weights {
		v["weights"] = json!(w);
	}
	if let Some(h) = o.holdout {
		v["holdout"] = json!(h);
	}
	v
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn encodes_as_javascript_does() {
		assert_eq!(uri_component(r#"{"a": "b c/é"}"#), "%7B%22a%22%3A%20%22b%20c%2F%C3%A9%22%7D");
		assert_eq!(uri_component("A-z_0.!~*'()"), "A-z_0.!~*'()");
	}
}
