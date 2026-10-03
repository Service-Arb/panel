//! The projections — `leads`, `calls`, `payments`, and the counts of
//! [`super::metrics`] — derived from registered events only.
//!
//! A lead is never patched: every event about it recomputes its row from all of its
//! events ([`panel_core::lead::fold`]), inside the transaction that journaled the event. That
//! transaction holds the database's one write lock ([`super::begin_write`]), so two events of
//! one lead arriving at once cannot each compute without the other. The rebuild calls the
//! very same function, which is why it lands on the same state.

use eyre::WrapErr;
use panel_core::{
	fact::Fact,
	ids::{BrandId, JobId, LeadId, LocationId},
	lead::{self, LeadState, Recorded},
};
use sqlx::SqliteConnection;

use super::{
	events::{self, Stored},
	to_db,
};
use crate::wire::Checked;

/// Projects one registered event: its call or payment row, and its lead.
pub async fn apply(conn: &mut SqliteConnection, event: &Recorded) -> eyre::Result<()> {
	insert_row(conn, event).await?;
	if let Some(lead) = &event.subject.lead_id {
		recompute_lead(conn, &event.subject.brand_id, lead).await?;
	}
	Ok(())
}

/// The event's own row, if its type has a table: a call, a payment, a count. Idempotent.
pub async fn insert_row(conn: &mut SqliteConnection, e: &Recorded) -> eyre::Result<()> {
	let lead = e.subject.lead_id.as_ref().map(LeadId::as_str);
	let location = e.subject.location_id.as_ref().map(LocationId::as_str);
	let manual = e.source_kind.is_manual();
	let at = to_db(e.occurred_at);
	match &e.fact {
		Fact::CallAttempted | Fact::CallLogged { .. } => {
			let (kind, outcome, attempt_id) = match &e.fact {
				Fact::CallLogged { outcome, attempt_id } => ("logged", Some(outcome.as_str()), attempt_id.as_deref()),
				_ => ("attempted", None, None),
			};
			sqlx::query(
				"INSERT INTO calls (event_id, brand_id, lead_id, location_id, kind, outcome, attempt_id, occurred_at, manual) \
				 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9) ON CONFLICT (event_id) DO NOTHING",
			)
			.bind(e.id.raw())
			.bind(e.subject.brand_id.as_str())
			.bind(lead)
			.bind(location)
			.bind(kind)
			.bind(outcome)
			.bind(attempt_id)
			.bind(at)
			.bind(manual)
			.execute(&mut *conn)
			.await
			.wrap_err("projecting a call")?;
		}
		Fact::PaymentReceived { billed, commission } => {
			sqlx::query(
				"INSERT INTO payments (event_id, brand_id, lead_id, location_id, job_id, billed, commission, currency, occurred_at, manual) \
				 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10) ON CONFLICT (event_id) DO NOTHING",
			)
			.bind(e.id.raw())
			.bind(e.subject.brand_id.as_str())
			.bind(lead)
			.bind(location)
			.bind(e.subject.job_id.as_ref().map(JobId::as_str))
			.bind(billed.minor)
			.bind(commission)
			.bind(billed.currency.as_str())
			.bind(at)
			.bind(manual)
			.execute(&mut *conn)
			.await
			.wrap_err("projecting a payment")?;
		}
		Fact::Metric(m) => super::metrics::project(conn, e, m).await?,
		Fact::LeadCreated { .. } | Fact::LeadContacted { .. } | Fact::LeadQuoted { .. } | Fact::JobWon | Fact::LeadLost { .. } | Fact::JobCompleted => {}
	}
	Ok(())
}

/// Recomputes a lead's row from all its registered events. Must run inside a write
/// transaction ([`super::begin_write`]), so no other write lands between the read and the
/// row.
pub async fn recompute_lead(conn: &mut SqliteConnection, brand: &BrandId, lead: &LeadId) -> eyre::Result<()> {
	let recorded: Vec<Recorded> = events::of_lead(conn, brand, lead).await?.into_iter().filter_map(registered).collect();
	match lead::fold(&recorded) {
		Some(state) => upsert_lead(conn, &state).await,
		None => {
			sqlx::query("DELETE FROM leads WHERE brand_id = $1 AND lead_id = $2")
				.bind(brand.as_str())
				.bind(lead.as_str())
				.execute(&mut *conn)
				.await
				.wrap_err("dropping a lead with no registered events")?;
			Ok(())
		}
	}
}

/// A stored event as a fact, if the registry still takes it. One marked registered that no
/// longer passes is left out, loudly: the rebuild will mark it.
pub fn registered(e: Stored) -> Option<Recorded> {
	match e.check() {
		Checked::Registered(fact) => Some(Recorded {
			id: e.id,
			occurred_at: e.occurred_at,
			received_at: e.received_at,
			source_kind: e.source_kind,
			subject: e.subject,
			fact,
		}),
		other => {
			tracing::warn!(event = %e.id.raw(), r#type = %e.type_key, checked = ?other, "an event stored as registered no longer passes the registry; run rebuild-projections");
			None
		}
	}
}

async fn upsert_lead(conn: &mut SqliteConnection, s: &LeadState) -> eyre::Result<()> {
	let t = |ts: Option<jiff::Timestamp>| ts.map(to_db);
	sqlx::query(
		"INSERT INTO leads (brand_id, lead_id, location_id, job_id, stage, channel, manual, \
		 created_at, contacted_at, quoted_at, won_at, completed_at, paid_at, lost_at, lost_reason, last_event_id, last_event_at, suspect) \
		 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18) \
		 ON CONFLICT (brand_id, lead_id) DO UPDATE SET \
		 location_id = EXCLUDED.location_id, job_id = EXCLUDED.job_id, stage = EXCLUDED.stage, channel = EXCLUDED.channel, manual = EXCLUDED.manual, \
		 created_at = EXCLUDED.created_at, contacted_at = EXCLUDED.contacted_at, quoted_at = EXCLUDED.quoted_at, won_at = EXCLUDED.won_at, \
		 completed_at = EXCLUDED.completed_at, paid_at = EXCLUDED.paid_at, lost_at = EXCLUDED.lost_at, lost_reason = EXCLUDED.lost_reason, \
		 last_event_id = EXCLUDED.last_event_id, last_event_at = EXCLUDED.last_event_at, suspect = EXCLUDED.suspect",
	)
	.bind(s.brand_id.as_str())
	.bind(s.lead_id.as_str())
	.bind(s.location_id.as_ref().map(LocationId::as_str))
	.bind(s.job_id.as_ref().map(JobId::as_str))
	.bind(s.stage.as_str())
	.bind(s.channel.map(|c| c.as_str()))
	.bind(s.manual)
	.bind(t(s.times.created))
	.bind(t(s.times.contacted))
	.bind(t(s.times.quoted))
	.bind(t(s.times.won))
	.bind(t(s.times.completed))
	.bind(t(s.times.paid))
	.bind(t(s.times.lost))
	.bind(s.lost_reason.as_deref())
	.bind(s.last_event_id.raw())
	.bind(to_db(s.last_event_at))
	.bind(s.suspect.map(|m| m.as_str()))
	.execute(&mut *conn)
	.await
	.wrap_err_with(|| format!("writing lead {}/{}", s.brand_id, s.lead_id))?;
	Ok(())
}

/// Empties the projections, for a rebuild. Inside the rebuild's write transaction, so ingest
/// waits for it to commit rather than writing into a half-built state, and readers keep
/// seeing the projections as they were until then.
pub async fn clear(conn: &mut SqliteConnection) -> eyre::Result<()> {
	sqlx::raw_sql("DELETE FROM leads; DELETE FROM calls; DELETE FROM payments; DELETE FROM daily_location_metrics; DELETE FROM daily_experiment_metrics")
		.execute(&mut *conn)
		.await
		.wrap_err("clearing the projections")?;
	Ok(())
}
