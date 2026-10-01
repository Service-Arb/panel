//! The PostHog import (spec §3.4, §7): every hour, the last days of the landings' events
//! counted by HogQL, and each count that changed journaled as a `site.metrics`,
//! `contact.metrics` or `experiment.metrics` event of `source.kind = posthog`, no key.
//!
//! ```text
//! lease (posthog_import: one replica, once an hour) ─ HogQL ×3 ─ rows → counts per slice
//!   ─ compared with the projection ─ changed slices → events, next revision ─ journal
//! ```
//!
//! **What is counted** — the landings' own event names, nothing invented here:
//!
//! - visits (stage 3): kitstart's `location_page_view{brand_id, location_id, source}`, by
//!   source (`utm_source`, the referrer's host, `direct`, `internal`, `unknown`), at most
//!   [`MAX_SOURCES`](panel_core::metrics::MAX_SOURCES) per location and day;
//! - intents (stage 4): kitstart's `contact_intent_click{brand_id, location_id, channel}`;
//! - experiments: the landings' `experiment_exposed`, `experiment_contact{channel}` and
//!   `experiment_lead`, each with `{experiment, variant, forced}`; `forced: true` (QA) is
//!   left out. kitstart's own events drop `variant` (it is off their allow-list), so the
//!   lead per variant is `experiment_lead`, not `lead_form_submit`.
//!
//! **A count replaces the one before.** The journal is append-only, so a recount is a new
//! event with the next revision, written only when the number changed, and a slice the
//! recount no longer finds goes back to 0 the same way. Its id derives from (type, day,
//! brand, location, slice, revision) and its time is the day's start, so the same recount
//! written twice — two replicas across a lapsed lease — is a duplicate, not a second event.
//!
//! **Which brands.** Anyone holding a landing's public PostHog key can send events naming any
//! brand, so only the brands a source key of the panel writes for are imported.

use std::{
	collections::{BTreeMap, BTreeSet},
	future::Future,
};

use eyre::WrapErr;
use jiff::{SignedDuration, Timestamp, civil::Date, tz::TimeZone};
use panel_contracts::SCHEMA;
use panel_core::{
	ids::LocationId,
	metrics::{IntentChannel, Tally, changes, normalize_source, top_sources},
};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::{
	Outcome, Panel,
	store::metrics::{self as db, CONTACT_INTENT, ExperimentSlice, LocationSlice, VISITS},
};

/// How many days back each import counts again, today included: PostHog takes late events,
/// and a landing's beacon may arrive after the day it was sent.
pub const WINDOW_DAYS: u16 = 3;

/// The most days one import may cover (a backfill from the CLI).
pub const MAX_DAYS: u16 = 400;

/// An import is due this long after the last one finished.
pub const EVERY: SignedDuration = SignedDuration::from_hours(1);

/// A failed import is tried again after this long.
pub const RETRY_AFTER: SignedDuration = SignedDuration::from_mins(10);

/// How long an import may hold the lease; longer than three HogQL calls and the writes.
pub const LEASE: SignedDuration = SignedDuration::from_mins(10);

/// The most rows one query may answer; a query that fills it is refused rather than read as
/// complete (HogQL answers 100 rows unless told otherwise).
pub const ROW_LIMIT: usize = 10_000;

/// PostHog's query API: a HogQL query and its placeholders' values in, the result table out.
pub trait Hogql: Sync {
	fn query(&self, hogql: &str, values: &Value) -> impl Future<Output = eyre::Result<Vec<Vec<Value>>>> + Send;
}

/// The three queries. `{from}` is the window's first instant, an RFC 3339 string; days are
/// UTC whatever the project's time zone.
pub const VISITS_HOGQL: &str = "SELECT formatDateTime(timestamp, '%Y-%m-%d', 'UTC') AS day, properties.brand_id AS brand, \
	properties.location_id AS location, properties.source AS source, count() AS n \
	FROM events WHERE event = 'location_page_view' AND timestamp >= toDateTime({from}) \
	GROUP BY day, brand, location, source LIMIT 10000";

pub const INTENTS_HOGQL: &str = "SELECT formatDateTime(timestamp, '%Y-%m-%d', 'UTC') AS day, properties.brand_id AS brand, \
	properties.location_id AS location, properties.channel AS channel, count() AS n \
	FROM events WHERE event = 'contact_intent_click' AND timestamp >= toDateTime({from}) \
	GROUP BY day, brand, location, channel LIMIT 10000";

pub const EXPERIMENTS_HOGQL: &str = "SELECT formatDateTime(timestamp, '%Y-%m-%d', 'UTC') AS day, properties.brand_id AS brand, \
	properties.experiment AS experiment, properties.variant AS variant, event, properties.channel AS channel, count() AS n \
	FROM events WHERE event IN ('experiment_exposed', 'experiment_contact', 'experiment_lead') \
	AND coalesce(toString(properties.forced), '') NOT IN ('true', '1') AND timestamp >= toDateTime({from}) \
	GROUP BY day, brand, experiment, variant, event, channel LIMIT 10000";

/// What an import did.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Imported {
	/// The days counted, first and last.
	pub from: Option<Date>,
	pub to: Option<Date>,
	/// Counts journaled: new, changed or gone back to 0.
	pub written: u64,
	/// Rows left out: another brand, a malformed day, location, name or channel.
	pub skipped: u64,
	/// Counts another writer had journaled first under the same revision with another number;
	/// the next import writes the one after.
	pub conflicts: u64,
}

/// Where a row stands with the window and the brands.
struct Scope {
	from: Date,
	to: Date,
	brands: BTreeSet<String>,
}

impl Scope {
	fn day(&self, v: Option<&Value>) -> Option<Date> {
		let d: Date = v?.as_str()?.parse().ok()?;
		(self.from..=self.to).contains(&d).then_some(d)
	}

	fn brand(&self, v: Option<&Value>) -> Option<String> {
		let b = v?.as_str()?;
		self.brands.contains(b).then(|| b.to_owned())
	}
}

/// A cell that may be absent: `Ok(None)` for null, `Err` for anything unfit.
fn location(v: Option<&Value>) -> Result<Option<String>, ()> {
	match v {
		None | Some(Value::Null) => Ok(None),
		Some(Value::String(s)) if s.is_empty() => Ok(None),
		Some(Value::String(s)) => LocationId::parse(s).map(|l| Some(l.as_str().to_owned())).map_err(drop),
		Some(_) => Err(()),
	}
}

fn text(v: Option<&Value>) -> Option<&str> {
	v?.as_str().filter(|s| !s.is_empty())
}

fn n(v: Option<&Value>) -> Option<u64> {
	match v? {
		Value::Number(n) => n.as_u64(),
		Value::String(s) => s.parse().ok(),
		_ => None,
	}
}

/// The fresh counts of the window, as the projection keys them.
#[derive(Debug, Default)]
struct Fresh {
	locations: BTreeMap<LocationSlice, u64>,
	experiments: BTreeMap<ExperimentSlice, Tally>,
	skipped: u64,
}

impl Fresh {
	fn visits(&mut self, scope: &Scope, rows: &[Vec<Value>]) {
		let mut by_place: BTreeMap<(Date, String, Option<String>), BTreeMap<String, u64>> = BTreeMap::new();
		for r in rows {
			let (Some(day), Some(brand), Ok(loc), Some(count)) = (scope.day(r.first()), scope.brand(r.get(1)), location(r.get(2)), n(r.get(4))) else {
				self.skipped += 1;
				continue;
			};
			let source = normalize_source(text(r.get(3)));
			*by_place.entry((day, brand, loc)).or_default().entry(source).or_insert(0) += count;
		}
		for ((day, brand, loc), sources) in by_place {
			for (source, count) in top_sources(sources) {
				self.locations.insert((day, brand.clone(), loc.clone(), VISITS, source), count);
			}
		}
	}

	fn intents(&mut self, scope: &Scope, rows: &[Vec<Value>]) {
		for r in rows {
			let channel = text(r.get(3)).and_then(|c| IntentChannel::parse(c).ok());
			let (Some(day), Some(brand), Ok(loc), Some(channel), Some(count)) = (scope.day(r.first()), scope.brand(r.get(1)), location(r.get(2)), channel, n(r.get(4))) else {
				self.skipped += 1;
				continue;
			};
			*self.locations.entry((day, brand, loc, CONTACT_INTENT, channel.as_str().to_owned())).or_insert(0) += count;
		}
	}

	fn experiments(&mut self, scope: &Scope, rows: &[Vec<Value>]) {
		for r in rows {
			let name = |i: usize| text(r.get(i)).filter(|s| panel_core::ids::is_slug(s)).map(str::to_owned);
			let (Some(day), Some(brand), Some(experiment), Some(variant), Some(count)) = (scope.day(r.first()), scope.brand(r.get(1)), name(2), name(3), n(r.get(6))) else {
				self.skipped += 1;
				continue;
			};
			let channel = text(r.get(5)).and_then(|c| IntentChannel::parse(c).ok());
			let mut add = Tally::default();
			match (text(r.get(4)), channel) {
				(Some("experiment_exposed"), _) => add.exposures = count,
				(Some("experiment_lead"), _) => add.leads = count,
				(Some("experiment_contact"), Some(c)) => *add.intents_mut(c) = count,
				_ => {
					self.skipped += 1;
					continue;
				}
			}
			self.experiments.entry((day, brand, experiment, variant)).or_default().add(&add);
		}
	}
}

/// The midnight a UTC day starts at.
fn day_start(d: Date) -> eyre::Result<Timestamp> {
	Ok(d.to_zoned(TimeZone::UTC).wrap_err_with(|| format!("the start of {d}"))?.timestamp())
}

impl Panel {
	/// Takes the import's lease for `holder` when an import is due (`force`: whenever no one
	/// holds it, for a one-off from the CLI).
	pub async fn posthog_import_lease(&self, holder: Uuid, now: Timestamp, force: bool) -> eyre::Result<bool> {
		let mut conn = self.store.pool().acquire().await.wrap_err("a connection for the import")?;
		let (due, retry) = if force { (now, now) } else { (now - EVERY, now - RETRY_AFTER) };
		db::import_lease(&mut conn, holder, now, now + LEASE, due, retry).await
	}

	/// Lets the lease go; `imported`: the import finished, at `now`.
	pub async fn posthog_import_release(&self, holder: Uuid, now: Timestamp, imported: bool) -> eyre::Result<()> {
		let mut conn = self.store.pool().acquire().await.wrap_err("a connection for the import")?;
		db::import_release(&mut conn, holder, now, imported).await
	}

	/// When the last import finished; `None` before the first.
	pub async fn posthog_imported_at(&self) -> eyre::Result<Option<Timestamp>> {
		let mut conn = self.store.pool().acquire().await.wrap_err("a connection")?;
		db::imported_at(&mut conn).await
	}

	/// Counts the last `days` UTC days (today included) in PostHog and journals what changed.
	/// `source_id` names the import in the journal (`posthog-<project>`). The caller holds the
	/// lease.
	pub async fn import_posthog(&self, api: &impl Hogql, source_id: &str, days: u16, now: Timestamp) -> eyre::Result<Imported> {
		eyre::ensure!((1..=MAX_DAYS).contains(&days), "an import covers 1 to {MAX_DAYS} days");
		let to = now.to_zoned(TimeZone::UTC).date();
		let from = to.checked_sub(SignedDuration::from_hours(24 * i64::from(days - 1))).wrap_err("the window's first day")?;
		let brands: BTreeSet<String> = self
			.store
			.sources()
			.await?
			.into_iter()
			.filter(|s| s.revoked_at.is_none())
			.flat_map(|s| s.grant.brands.into_iter().map(|b| b.as_str().to_owned()))
			.collect();
		let mut done = Imported {
			from: Some(from),
			to: Some(to),
			..Imported::default()
		};
		if brands.is_empty() {
			tracing::warn!("posthog import: no source key names a brand yet; nothing to import");
			return Ok(done);
		}
		let values = json!({ "from": day_start(from)?.to_string() });
		let scope = Scope { from, to, brands };
		let mut fresh = Fresh::default();
		fresh.visits(&scope, &rows(api, VISITS_HOGQL, &values, "visits").await?);
		fresh.intents(&scope, &rows(api, INTENTS_HOGQL, &values, "intents").await?);
		fresh.experiments(&scope, &rows(api, EXPERIMENTS_HOGQL, &values, "experiments").await?);
		done.skipped = fresh.skipped;

		let brand_list: Vec<&str> = scope.brands.iter().map(String::as_str).collect();
		let mut conn = self.store.pool().acquire().await.wrap_err("a connection for the import")?;
		let had_locations = db::location_counts_since(&mut conn, from, &brand_list).await?;
		let had_experiments = db::experiment_counts_since(&mut conn, from, &brand_list).await?;
		drop(conn);

		let mut events = Vec::new();
		for ((day, brand, loc, metric, dimension), value, revision) in changes(&had_locations, &fresh.locations, &0) {
			let (r#type, properties) = match metric {
				VISITS => ("site.metrics", json!({"day": day.to_string(), "source": dimension, "visits": value, "revision": revision})),
				_ => ("contact.metrics", json!({"day": day.to_string(), "channel": dimension, "intents": value, "revision": revision})),
			};
			let id = crate::derived_id(
				b"sa-panel/posthog/v1/",
				&[
					r#type.as_bytes(),
					day.to_string().as_bytes(),
					brand.as_bytes(),
					loc.as_deref().unwrap_or("").as_bytes(),
					dimension.as_bytes(),
					&revision.to_be_bytes(),
				],
			);
			let mut subject = json!({"brandId": brand});
			if let Some(loc) = loc {
				subject["locationId"] = json!(loc);
			}
			events.push((id, day, r#type, subject, properties));
		}
		for ((day, brand, experiment, variant), t, revision) in changes(&had_experiments, &fresh.experiments, &Tally::default()) {
			let id = crate::derived_id(
				b"sa-panel/posthog/v1/",
				&[
					b"experiment.metrics",
					day.to_string().as_bytes(),
					brand.as_bytes(),
					experiment.as_bytes(),
					variant.as_bytes(),
					&revision.to_be_bytes(),
				],
			);
			let properties = json!({
				"day": day.to_string(), "experiment": experiment, "variant": variant, "exposures": t.exposures, "leads": t.leads,
				"phone": t.phone, "whatsapp": t.whatsapp, "formOpen": t.form_open, "booking": t.booking, "revision": revision,
			});
			events.push((id, day, "experiment.metrics", json!({"brandId": brand}), properties));
		}

		for (id, day, r#type, subject, properties) in events {
			let raw = json!({
				"id": id.to_string(),
				"schema": SCHEMA,
				"type": r#type,
				"typeVersion": 1,
				"occurredAt": day_start(day)?.to_string(),
				"source": {"kind": "posthog", "id": source_id},
				"subject": subject,
				"properties": properties,
			});
			match self.write_own(raw, now).await? {
				Ok((Outcome::Accepted { .. }, _)) => done.written += 1,
				// The same recount, written by another replica first.
				Ok((Outcome::Duplicate, _)) => {}
				Ok((Outcome::Rejected(e), env)) => {
					tracing::warn!(r#type, brand = %env.subject.brand_id, reason = %e, "posthog import: a count was journaled first under the same revision");
					done.conflicts += 1;
				}
				// What the import builds must pass the registry: a refusal is a bug here.
				Err(e) => eyre::bail!("the import made a {} count the registry refuses: {e}", r#type),
			}
		}
		if done.skipped > 0 {
			tracing::warn!(
				skipped = done.skipped,
				"posthog import: rows left out (unknown brand, or a malformed day, location, name or channel)"
			);
		}
		tracing::info!(%from, %to, written = done.written, conflicts = done.conflicts, "posthog import done");
		Ok(done)
	}
}

async fn rows(api: &impl Hogql, hogql: &str, values: &Value, what: &str) -> eyre::Result<Vec<Vec<Value>>> {
	let rows = api.query(hogql, values).await.wrap_err_with(|| format!("querying PostHog for the {what}"))?;
	eyre::ensure!(rows.len() < ROW_LIMIT, "PostHog answered {ROW_LIMIT} rows for the {what}, its limit: import fewer days at once");
	Ok(rows)
}

#[cfg(test)]
mod tests {
	use super::*;

	fn scope() -> Scope {
		Scope {
			from: "2026-09-28".parse().unwrap(),
			to: "2026-09-30".parse().unwrap(),
			brands: ["aquafix".to_owned()].into(),
		}
	}

	#[test]
	fn rows_are_read_or_left_out() {
		let s = scope();
		let mut f = Fresh::default();
		f.visits(
			&s,
			&[
				vec![json!("2026-09-29"), json!("aquafix"), json!("paris-11"), json!("Google.com"), json!(5)],
				vec![json!("2026-09-29"), json!("aquafix"), json!("paris-11"), json!("google.com"), json!("2")],
				vec![json!("2026-09-29"), json!("aquafix"), Value::Null, Value::Null, json!(1)],
				vec![json!("2026-09-27"), json!("aquafix"), json!("paris-11"), json!("direct"), json!(1)],
				vec![json!("2026-09-29"), json!("vifnet"), json!("paris-11"), json!("direct"), json!(1)],
				vec![json!("2026-09-29"), json!("aquafix"), json!("a b"), json!("direct"), json!(1)],
			],
		);
		let day: Date = "2026-09-29".parse().unwrap();
		assert_eq!(
			f.locations[&(day, "aquafix".into(), Some("paris-11".into()), VISITS, "google.com".into())],
			7,
			"one source, spelt two ways"
		);
		assert_eq!(f.locations[&(day, "aquafix".into(), None, VISITS, "unknown".into())], 1);
		assert_eq!(f.skipped, 3, "out of the window, another brand, a location that is not one");

		f.intents(
			&s,
			&[
				vec![json!("2026-09-30"), json!("aquafix"), json!("lyon-2"), json!("phone"), json!(3)],
				vec![json!("2026-09-30"), json!("aquafix"), json!("lyon-2"), json!("fax"), json!(3)],
			],
		);
		assert_eq!(f.locations.iter().filter(|(k, _)| k.3 == CONTACT_INTENT).count(), 1);
		assert_eq!(f.skipped, 4);

		f.experiments(
			&s,
			&[
				vec![
					json!("2026-09-30"),
					json!("aquafix"),
					json!("hero"),
					json!("b"),
					json!("experiment_exposed"),
					Value::Null,
					json!(100),
				],
				vec![
					json!("2026-09-30"),
					json!("aquafix"),
					json!("hero"),
					json!("b"),
					json!("experiment_contact"),
					json!("whatsapp"),
					json!(4),
				],
				vec![json!("2026-09-30"), json!("aquafix"), json!("hero"), json!("b"), json!("experiment_lead"), Value::Null, json!(2)],
				vec![json!("2026-09-30"), json!("aquafix"), json!("hero"), json!("b"), json!("experiment_step"), Value::Null, json!(9)],
			],
		);
		let t = f.experiments[&("2026-09-30".parse().unwrap(), "aquafix".into(), "hero".into(), "b".into())];
		assert_eq!((t.exposures, t.whatsapp, t.leads), (100, 4, 2));
		assert_eq!(f.skipped, 5, "steps are not counted");
	}
}
