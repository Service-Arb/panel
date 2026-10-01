//! The counts: `daily_location_metrics` (visits by source, intents by channel) and
//! `daily_experiment_metrics`, each row the highest revision journaled of its slice; what the
//! screens read of them through `reporting`; and the PostHog import's lease.

use std::collections::BTreeMap;

use chrono::{DateTime, NaiveDate, Utc};
use eyre::WrapErr;
use jiff::{Timestamp, civil::Date};
use panel_core::{
	ids::{BrandId, LocationId},
	lead::Recorded,
	metrics::{DailyMetric, IntentChannel, MetricValue, Tally},
};
use sqlx::PgConnection;
use uuid::Uuid;

use super::{from_pg, from_pg_day, to_pg, to_pg_day};

/// What a location's count is of, as the projection names it.
pub const VISITS: &str = "visits";
pub const CONTACT_INTENT: &str = "contact_intent";

/// Projects a count: its slice's row, unless that row already holds this revision or a
/// later one — so the journal can be replayed in any order and land on the newest count.
pub async fn project(conn: &mut PgConnection, e: &Recorded, m: &DailyMetric) -> eyre::Result<()> {
	let day = to_pg_day(m.day)?;
	let revision = i32::try_from(m.revision).wrap_err("revision past i32")?;
	let brand = e.subject.brand_id.as_str();
	let (metric, value) = match &m.value {
		MetricValue::Visits { visits, .. } => (VISITS, *visits),
		MetricValue::Intents { intents, .. } => (CONTACT_INTENT, *intents),
		MetricValue::Experiment { experiment, variant, tally } => {
			sqlx::query(
				"INSERT INTO daily_experiment_metrics (day, brand_id, experiment, variant, exposures, leads, phone, whatsapp, form_open, booking, revision, event_id) \
				 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12) \
				 ON CONFLICT (day, brand_id, experiment, variant) DO UPDATE \
				 SET exposures = EXCLUDED.exposures, leads = EXCLUDED.leads, phone = EXCLUDED.phone, whatsapp = EXCLUDED.whatsapp, \
				 form_open = EXCLUDED.form_open, booking = EXCLUDED.booking, revision = EXCLUDED.revision, event_id = EXCLUDED.event_id \
				 WHERE daily_experiment_metrics.revision < EXCLUDED.revision",
			)
			.bind(day)
			.bind(brand)
			.bind(experiment)
			.bind(variant)
			.bind(big(tally.exposures)?)
			.bind(big(tally.leads)?)
			.bind(big(tally.phone)?)
			.bind(big(tally.whatsapp)?)
			.bind(big(tally.form_open)?)
			.bind(big(tally.booking)?)
			.bind(revision)
			.bind(e.id.raw())
			.execute(&mut *conn)
			.await
			.wrap_err("projecting an experiment's count")?;
			return Ok(());
		}
	};
	sqlx::query(
		"INSERT INTO daily_location_metrics (day, brand_id, location_id, metric, dimension, value, revision, event_id) \
				 VALUES ($1, $2, $3, $4, $5, $6, $7, $8) \
				 ON CONFLICT ON CONSTRAINT daily_location_metrics_slice DO UPDATE \
				 SET value = EXCLUDED.value, revision = EXCLUDED.revision, event_id = EXCLUDED.event_id \
				 WHERE daily_location_metrics.revision < EXCLUDED.revision",
	)
	.bind(day)
	.bind(brand)
	.bind(e.subject.location_id.as_ref().map(LocationId::as_str))
	.bind(metric)
	.bind(m.dimension())
	.bind(big(value)?)
	.bind(revision)
	.bind(e.id.raw())
	.execute(&mut *conn)
	.await
	.wrap_err("projecting a location's count")?;
	Ok(())
}

fn big(v: u64) -> eyre::Result<i64> {
	i64::try_from(v).wrap_err("a count past i64")
}

fn count(v: i64) -> eyre::Result<u64> {
	u64::try_from(v).wrap_err("a negative count")
}

fn revision(v: i32) -> eyre::Result<u32> {
	u32::try_from(v).wrap_err("a negative revision")
}

/// A location count's slice: day, brand, location, metric, dimension.
pub type LocationSlice = (Date, String, Option<String>, &'static str, String);

/// An experiment count's slice: day, brand, experiment, variant.
pub type ExperimentSlice = (Date, String, String, String);

/// Every location count of `brands` from `from` on, with its revision: what a fresh import is
/// compared with.
pub async fn location_counts_since(conn: &mut PgConnection, from: Date, brands: &[&str]) -> eyre::Result<BTreeMap<LocationSlice, (u64, u32)>> {
	type Row = (NaiveDate, String, Option<String>, String, String, i64, i32);
	let rows: Vec<Row> = sqlx::query_as(
		"SELECT day, brand_id, location_id, metric, dimension, value, revision FROM daily_location_metrics \
		 WHERE day >= $1 AND brand_id = ANY($2)",
	)
	.bind(to_pg_day(from)?)
	.bind(brands)
	.fetch_all(&mut *conn)
	.await
	.wrap_err("reading the location counts")?;
	rows.into_iter()
		.map(|(day, brand, location, metric, dimension, value, rev)| {
			let metric = match metric.as_str() {
				VISITS => VISITS,
				CONTACT_INTENT => CONTACT_INTENT,
				other => eyre::bail!("stored metric {other:?}"),
			};
			Ok(((from_pg_day(day)?, brand, location, metric, dimension), (count(value)?, revision(rev)?)))
		})
		.collect()
}

/// Every experiment count of `brands` from `from` on, with its revision.
pub async fn experiment_counts_since(conn: &mut PgConnection, from: Date, brands: &[&str]) -> eyre::Result<BTreeMap<ExperimentSlice, (Tally, u32)>> {
	type Row = (NaiveDate, String, String, String, i64, i64, i64, i64, i64, i64, i32);
	let rows: Vec<Row> = sqlx::query_as(
		"SELECT day, brand_id, experiment, variant, exposures, leads, phone, whatsapp, form_open, booking, revision \
		 FROM daily_experiment_metrics WHERE day >= $1 AND brand_id = ANY($2)",
	)
	.bind(to_pg_day(from)?)
	.bind(brands)
	.fetch_all(&mut *conn)
	.await
	.wrap_err("reading the experiment counts")?;
	rows.into_iter()
		.map(|r| {
			let tally = Tally {
				exposures: count(r.4)?,
				leads: count(r.5)?,
				phone: count(r.6)?,
				whatsapp: count(r.7)?,
				form_open: count(r.8)?,
				booking: count(r.9)?,
			};
			Ok(((from_pg_day(r.0)?, r.1, r.2, r.3), (tally, revision(r.10)?)))
		})
		.collect()
}

/// One day's sum of a slice, as the screens read it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocationDay {
	pub brand_id: String,
	pub location_id: Option<String>,
	pub day: Date,
	pub metric: &'static str,
	pub dimension: String,
	pub value: u64,
}

/// The location counts from `from` to `to` (both included), from `reporting`.
pub async fn location_days(conn: &mut PgConnection, from: Date, to: Date, brand: Option<&BrandId>) -> eyre::Result<Vec<LocationDay>> {
	type Row = (String, Option<String>, NaiveDate, String, String, i64);
	let rows: Vec<Row> = sqlx::query_as(
		"SELECT brand_id, location_id, day, metric, dimension, value FROM reporting.daily_location_metrics \
		 WHERE day BETWEEN $1 AND $2 AND ($3::text IS NULL OR brand_id = $3) \
		 ORDER BY brand_id, location_id NULLS LAST, day, metric, dimension",
	)
	.bind(to_pg_day(from)?)
	.bind(to_pg_day(to)?)
	.bind(brand.map(BrandId::as_str))
	.fetch_all(&mut *conn)
	.await
	.wrap_err("reading the location counts")?;
	rows.into_iter()
		.map(|(brand_id, location_id, day, metric, dimension, value)| {
			let metric = match metric.as_str() {
				VISITS => VISITS,
				CONTACT_INTENT => {
					IntentChannel::parse(&dimension).wrap_err("a stored channel")?;
					CONTACT_INTENT
				}
				other => eyre::bail!("stored metric {other:?}"),
			};
			Ok(LocationDay {
				brand_id,
				location_id,
				day: from_pg_day(day)?,
				metric,
				dimension,
				value: count(value)?,
			})
		})
		.collect()
}

/// One variant's counts summed over some days.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VariantTotals {
	pub brand_id: String,
	pub experiment: String,
	pub variant: String,
	pub first_day: Date,
	pub last_day: Date,
	pub tally: Tally,
}

/// Every variant counted from `from` to `to` (both included), summed, from `reporting`.
pub async fn variant_totals(conn: &mut PgConnection, from: Date, to: Date, brand: Option<&BrandId>) -> eyre::Result<Vec<VariantTotals>> {
	type Row = (String, String, String, NaiveDate, NaiveDate, i64, i64, i64, i64, i64, i64);
	let rows: Vec<Row> = sqlx::query_as(
		"SELECT brand_id, experiment, variant, min(day), max(day), sum(exposures)::bigint, sum(leads)::bigint, \
		 sum(phone)::bigint, sum(whatsapp)::bigint, sum(form_open)::bigint, sum(booking)::bigint \
		 FROM reporting.experiment_daily WHERE day BETWEEN $1 AND $2 AND ($3::text IS NULL OR brand_id = $3) \
		 GROUP BY 1, 2, 3 ORDER BY 1, 2, 3",
	)
	.bind(to_pg_day(from)?)
	.bind(to_pg_day(to)?)
	.bind(brand.map(BrandId::as_str))
	.fetch_all(&mut *conn)
	.await
	.wrap_err("summing the experiments")?;
	rows.into_iter()
		.map(|r| {
			Ok(VariantTotals {
				brand_id: r.0,
				experiment: r.1,
				variant: r.2,
				first_day: from_pg_day(r.3)?,
				last_day: from_pg_day(r.4)?,
				tally: Tally {
					exposures: count(r.5)?,
					leads: count(r.6)?,
					phone: count(r.7)?,
					whatsapp: count(r.8)?,
					form_open: count(r.9)?,
					booking: count(r.10)?,
				},
			})
		})
		.collect()
}

// ── the import's lease ──────────────────────────────────────────────────────────────────

/// Takes the import for `holder` until `until`, if no one holds it and it is due: last
/// finished at or before `due_before`, last tried at or before `retry_before`.
pub async fn import_lease(conn: &mut PgConnection, holder: Uuid, now: Timestamp, until: Timestamp, due_before: Timestamp, retry_before: Timestamp) -> eyre::Result<bool> {
	let taken = sqlx::query(
		"UPDATE posthog_import SET holder = $1, leased_until = $3, attempted_at = $2 \
		 WHERE (holder IS NULL OR holder = $1 OR leased_until IS NULL OR leased_until <= $2) \
		 AND (imported_at IS NULL OR imported_at <= $4) AND (attempted_at IS NULL OR attempted_at <= $5)",
	)
	.bind(holder)
	.bind(to_pg(now)?)
	.bind(to_pg(until)?)
	.bind(to_pg(due_before)?)
	.bind(to_pg(retry_before)?)
	.execute(&mut *conn)
	.await
	.wrap_err("leasing the import")?
	.rows_affected();
	Ok(taken == 1)
}

/// Lets the lease go; `imported` records the import as finished at `now`.
pub async fn import_release(conn: &mut PgConnection, holder: Uuid, now: Timestamp, imported: bool) -> eyre::Result<()> {
	sqlx::query(
		"UPDATE posthog_import SET holder = NULL, leased_until = NULL, imported_at = CASE WHEN $3 THEN $2 ELSE imported_at END \
		 WHERE holder = $1",
	)
	.bind(holder)
	.bind(to_pg(now)?)
	.bind(imported)
	.execute(&mut *conn)
	.await
	.wrap_err("releasing the import")?;
	Ok(())
}

/// When the last import finished.
pub async fn imported_at(conn: &mut PgConnection) -> eyre::Result<Option<Timestamp>> {
	let at: Option<DateTime<Utc>> = sqlx::query_scalar("SELECT imported_at FROM posthog_import")
		.fetch_one(&mut *conn)
		.await
		.wrap_err("reading when the import last finished")?;
	at.map(from_pg).transpose()
}
