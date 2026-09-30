//! What the operator screens read: leads with the PII of their creation, a lead's events,
//! and the funnel's daily totals from `reporting`.

use chrono::{DateTime, NaiveDate, Utc};
use eyre::WrapErr;
use jiff::Timestamp;
use panel_core::{
	funnel::Totals,
	ids::{BrandId, LeadId, LocationId},
	lead::Stage,
};
use serde_json::Value;
use sqlx::{PgConnection, types::Json};
use uuid::Uuid;

use super::{from_pg, to_pg};

/// A lead as the projection holds it, with its creating event's sealed PII.
#[derive(Clone, Debug)]
pub struct LeadRow {
	pub brand_id: String,
	pub lead_id: String,
	pub location_id: Option<String>,
	pub job_id: Option<String>,
	pub stage: Stage,
	pub channel: Option<String>,
	pub manual: bool,
	pub created_at: Option<Timestamp>,
	pub contacted_at: Option<Timestamp>,
	pub quoted_at: Option<Timestamp>,
	pub won_at: Option<Timestamp>,
	pub completed_at: Option<Timestamp>,
	pub paid_at: Option<Timestamp>,
	pub lost_at: Option<Timestamp>,
	pub lost_reason: Option<String>,
	pub last_event_at: Timestamp,
	/// When it sorts in the list: its creation, or its first event when never created.
	pub sort_at: Timestamp,
	/// The `lead.created` that counts (the first journaled), and its sealed PII.
	pub creation: Option<Sealed>,
}

/// An event's id and its sealed PII with the fingerprint of the key that sealed it.
#[derive(Clone, Debug)]
pub struct Sealed {
	pub event_id: Uuid,
	pub pii: Option<(Vec<u8>, Vec<u8>)>,
}

#[derive(sqlx::FromRow)]
struct Row {
	brand_id: String,
	lead_id: String,
	location_id: Option<String>,
	job_id: Option<String>,
	stage: String,
	channel: Option<String>,
	manual: bool,
	created_at: Option<DateTime<Utc>>,
	contacted_at: Option<DateTime<Utc>>,
	quoted_at: Option<DateTime<Utc>>,
	won_at: Option<DateTime<Utc>>,
	completed_at: Option<DateTime<Utc>>,
	paid_at: Option<DateTime<Utc>>,
	lost_at: Option<DateTime<Utc>>,
	lost_reason: Option<String>,
	last_event_at: DateTime<Utc>,
	sort_at: DateTime<Utc>,
	creation_id: Option<Uuid>,
	pii_sealed: Option<Vec<u8>>,
	data_key_fp: Option<Vec<u8>>,
}

impl TryFrom<Row> for LeadRow {
	type Error = eyre::Report;

	fn try_from(r: Row) -> eyre::Result<Self> {
		let t = |ts: Option<DateTime<Utc>>| ts.map(from_pg).transpose();
		Ok(Self {
			stage: r.stage.parse().wrap_err_with(|| format!("stored stage of lead {}/{}", r.brand_id, r.lead_id))?,
			created_at: t(r.created_at)?,
			contacted_at: t(r.contacted_at)?,
			quoted_at: t(r.quoted_at)?,
			won_at: t(r.won_at)?,
			completed_at: t(r.completed_at)?,
			paid_at: t(r.paid_at)?,
			lost_at: t(r.lost_at)?,
			last_event_at: from_pg(r.last_event_at)?,
			sort_at: from_pg(r.sort_at)?,
			creation: r.creation_id.map(|event_id| Sealed {
				event_id,
				pii: r.pii_sealed.zip(r.data_key_fp),
			}),
			brand_id: r.brand_id,
			lead_id: r.lead_id,
			location_id: r.location_id,
			job_id: r.job_id,
			channel: r.channel,
			manual: r.manual,
			lost_reason: r.lost_reason,
		})
	}
}

// Macros, not consts, so every query stays a literal (`concat!`) that sqlx takes as audited.
macro_rules! lead_select {
	() => {
		"SELECT l.brand_id, l.lead_id, l.location_id, l.job_id, l.stage, l.channel, l.manual, \
		 l.created_at, l.contacted_at, l.quoted_at, l.won_at, l.completed_at, l.paid_at, l.lost_at, l.lost_reason, l.last_event_at, \
		 COALESCE(l.created_at, l.last_event_at) AS sort_at, \
		 c.id AS creation_id, c.pii_sealed, c.data_key_fp \
		 FROM leads l \
		 LEFT JOIN LATERAL ( \
		   SELECT e.id, e.pii_sealed, e.data_key_fp FROM events e \
		   WHERE e.brand_id = l.brand_id AND e.lead_id = l.lead_id AND e.type = 'lead.created' AND e.status = 'registered' \
		   ORDER BY e.received_at, e.id LIMIT 1 \
		 ) c ON true "
	};
}

/// Which leads to list.
#[derive(Clone, Debug, Default)]
pub struct LeadFilter {
	pub stage: Option<Stage>,
	pub brand: Option<BrandId>,
	pub location: Option<LocationId>,
	/// Only leads created before this and still waiting for their first contact.
	pub waiting_since_before: Option<Timestamp>,
	/// The last lead of the page before: `(sort_at, brand, lead)`.
	pub after: Option<(Timestamp, String, String)>,
	pub limit: i64,
}

/// Newest first, by creation.
pub async fn leads(conn: &mut PgConnection, f: &LeadFilter) -> eyre::Result<Vec<LeadRow>> {
	let (after_at, after_brand, after_lead) = match &f.after {
		Some((at, brand, lead)) => (Some(to_pg(*at)?), Some(brand.as_str()), Some(lead.as_str())),
		None => (None, None, None),
	};
	sqlx::query_as::<_, Row>(concat!(
		lead_select!(),
		"WHERE ($1::text IS NULL OR l.stage = $1) AND ($2::text IS NULL OR l.brand_id = $2) AND ($3::text IS NULL OR l.location_id = $3) \
		 AND ($4::timestamptz IS NULL OR (l.stage = 'created' AND l.contacted_at IS NULL AND l.created_at < $4)) \
		 AND ($5::timestamptz IS NULL OR (COALESCE(l.created_at, l.last_event_at), l.brand_id, l.lead_id) < ($5, $6, $7)) \
		 ORDER BY sort_at DESC, l.brand_id DESC, l.lead_id DESC LIMIT $8"
	))
	.bind(f.stage.map(Stage::as_str))
	.bind(f.brand.as_ref().map(BrandId::as_str))
	.bind(f.location.as_ref().map(LocationId::as_str))
	.bind(f.waiting_since_before.map(to_pg).transpose()?)
	.bind(after_at)
	.bind(after_brand)
	.bind(after_lead)
	.bind(f.limit)
	.fetch_all(&mut *conn)
	.await
	.wrap_err("listing leads")?
	.into_iter()
	.map(LeadRow::try_from)
	.collect()
}

pub async fn lead(conn: &mut PgConnection, brand: &BrandId, lead: &LeadId) -> eyre::Result<Option<LeadRow>> {
	sqlx::query_as::<_, Row>(concat!(lead_select!(), "WHERE l.brand_id = $1 AND l.lead_id = $2"))
		.bind(brand.as_str())
		.bind(lead.as_str())
		.fetch_optional(&mut *conn)
		.await
		.wrap_err_with(|| format!("reading lead {brand}/{lead}"))?
		.map(LeadRow::try_from)
		.transpose()
}

/// One event of a lead, as its card shows it.
#[derive(Clone, Debug)]
pub struct EventRow {
	pub id: Uuid,
	pub r#type: String,
	pub type_version: i32,
	pub occurred_at: Timestamp,
	pub received_at: Timestamp,
	pub source_kind: String,
	pub source_id: String,
	pub job_id: Option<String>,
	pub status: String,
	pub status_reason: Option<String>,
	pub properties: Value,
	pub sealed: Sealed,
}

#[derive(sqlx::FromRow)]
struct EventDbRow {
	id: Uuid,
	r#type: String,
	type_version: i32,
	occurred_at: DateTime<Utc>,
	received_at: DateTime<Utc>,
	source_kind: String,
	source_id: String,
	job_id: Option<String>,
	status: String,
	status_reason: Option<String>,
	properties: Json<Value>,
	pii_sealed: Option<Vec<u8>>,
	data_key_fp: Option<Vec<u8>>,
}

/// Every event about a lead, whatever the registry made of it, in the order they happened.
pub async fn lead_events(conn: &mut PgConnection, brand: &BrandId, lead: &LeadId) -> eyre::Result<Vec<EventRow>> {
	let rows = sqlx::query_as::<_, EventDbRow>(
		"SELECT id, type, type_version, occurred_at, received_at, source_kind, source_id, job_id, status, status_reason, properties, pii_sealed, data_key_fp \
		 FROM events WHERE brand_id = $1 AND lead_id = $2 ORDER BY occurred_at, id",
	)
	.bind(brand.as_str())
	.bind(lead.as_str())
	.fetch_all(&mut *conn)
	.await
	.wrap_err_with(|| format!("reading the events of lead {brand}/{lead}"))?;
	rows.into_iter()
		.map(|r| {
			Ok(EventRow {
				occurred_at: from_pg(r.occurred_at)?,
				received_at: from_pg(r.received_at)?,
				sealed: Sealed {
					event_id: r.id,
					pii: r.pii_sealed.zip(r.data_key_fp),
				},
				id: r.id,
				r#type: r.r#type,
				type_version: r.type_version,
				source_kind: r.source_kind,
				source_id: r.source_id,
				job_id: r.job_id,
				status: r.status,
				status_reason: r.status_reason,
				properties: r.properties.0,
			})
		})
		.collect()
}

/// Who journaled the event `id`, and about which lead: `(source_kind, source_id, lead_id)`.
pub async fn event_origin(conn: &mut PgConnection, id: Uuid) -> eyre::Result<Option<(String, String, Option<String>)>> {
	sqlx::query_as("SELECT source_kind, source_id, lead_id FROM events WHERE id = $1")
		.bind(id)
		.fetch_optional(&mut *conn)
		.await
		.wrap_err("looking up an event's origin")
}

/// Whether `attempt` is a `call.attempted` of this lead.
pub async fn is_call_attempt(conn: &mut PgConnection, brand: &BrandId, lead: &LeadId, attempt: Uuid) -> eyre::Result<bool> {
	sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM calls WHERE event_id = $1 AND brand_id = $2 AND lead_id = $3 AND kind = 'attempted')")
		.bind(attempt)
		.bind(brand.as_str())
		.bind(lead.as_str())
		.fetch_one(&mut *conn)
		.await
		.wrap_err("looking up a call attempt")
}

/// The personal funnel's totals over the leads that came in from `from` to `to`, UTC days,
/// both included; for one brand or all.
pub async fn funnel(conn: &mut PgConnection, from: NaiveDate, to: NaiveDate, brand: Option<&BrandId>) -> eyre::Result<Totals> {
	type Sums = (i64, i64, i64, i64, i64, i64, i64, i64);
	let s: Sums = sqlx::query_as(
		"SELECT COALESCE(sum(leads), 0)::bigint, COALESCE(sum(contacted), 0)::bigint, COALESCE(sum(quoted), 0)::bigint, COALESCE(sum(won), 0)::bigint, \
		 COALESCE(sum(completed), 0)::bigint, COALESCE(sum(paid), 0)::bigint, COALESCE(sum(lost_now), 0)::bigint, COALESCE(sum(manual), 0)::bigint \
		 FROM reporting.funnel_daily WHERE day BETWEEN $1 AND $2 AND ($3::text IS NULL OR brand_id = $3)",
	)
	.bind(from)
	.bind(to)
	.bind(brand.map(BrandId::as_str))
	.fetch_one(&mut *conn)
	.await
	.wrap_err("summing the funnel")?;
	let n = |v: i64| u64::try_from(v).wrap_err("a negative count");
	Ok(Totals {
		leads: n(s.0)?,
		contacted: n(s.1)?,
		quoted: n(s.2)?,
		won: n(s.3)?,
		completed: n(s.4)?,
		paid: n(s.5)?,
		lost: n(s.6)?,
		manual: n(s.7)?,
	})
}
