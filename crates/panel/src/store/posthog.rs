//! `posthog_outbox`: the lead events PostHog is owed, queued with the journal's write and
//! deleted once sent ([`crate::capture`]).

use eyre::WrapErr;
use jiff::Timestamp;
use serde_json::Value;
use sqlx::{SqliteConnection, types::Json};
use uuid::Uuid;

use super::{from_db, to_db};

/// One event owed to PostHog.
#[derive(Clone, Debug, PartialEq)]
pub struct Queued {
	pub event_id: Uuid,
	pub event: String,
	pub distinct_id: String,
	pub properties: Value,
	pub occurred_at: Timestamp,
	pub queued_at: Timestamp,
	pub tries: u32,
}

/// Queues an event, in the journal's transaction; nothing if it is queued already.
pub async fn enqueue(conn: &mut SqliteConnection, q: &Queued) -> eyre::Result<()> {
	sqlx::query(
		"INSERT INTO posthog_outbox (event_id, event, distinct_id, properties, occurred_at, queued_at, next_attempt_at) \
		 VALUES ($1, $2, $3, $4, $5, $6, $6) ON CONFLICT (event_id) DO NOTHING",
	)
	.bind(q.event_id)
	.bind(&q.event)
	.bind(&q.distinct_id)
	.bind(Json(&q.properties))
	.bind(to_db(q.occurred_at))
	.bind(to_db(q.queued_at))
	.execute(&mut *conn)
	.await
	.wrap_err("queueing an event for PostHog")?;
	Ok(())
}

/// What is due at `now`, oldest attempt first, at most `limit`.
pub async fn due(conn: &mut SqliteConnection, now: Timestamp, limit: i64) -> eyre::Result<Vec<Queued>> {
	type Row = (Uuid, String, String, Json<Value>, i64, i64, i64);
	let rows: Vec<Row> = sqlx::query_as(
		"SELECT event_id, event, distinct_id, properties, occurred_at, queued_at, tries FROM posthog_outbox \
		 WHERE next_attempt_at <= $1 ORDER BY next_attempt_at, event_id LIMIT $2",
	)
	.bind(to_db(now))
	.bind(limit)
	.fetch_all(&mut *conn)
	.await
	.wrap_err("reading what PostHog is owed")?;
	rows.into_iter()
		.map(|(event_id, event, distinct_id, properties, occurred_at, queued_at, tries)| {
			Ok(Queued {
				event_id,
				event,
				distinct_id,
				properties: properties.0,
				occurred_at: from_db(occurred_at)?,
				queued_at: from_db(queued_at)?,
				tries: u32::try_from(tries).wrap_err("stored tries")?,
			})
		})
		.collect()
}

/// Sent, or given up on: gone from the outbox.
pub async fn remove(conn: &mut SqliteConnection, event_id: Uuid) -> eyre::Result<()> {
	sqlx::query("DELETE FROM posthog_outbox WHERE event_id = $1")
		.bind(event_id)
		.execute(&mut *conn)
		.await
		.wrap_err("removing an event from the PostHog outbox")?;
	Ok(())
}

/// Tried and failed: tried again at `next`.
pub async fn retry(conn: &mut SqliteConnection, event_id: Uuid, tries: u32, next: Timestamp, error: &str) -> eyre::Result<()> {
	sqlx::query("UPDATE posthog_outbox SET tries = $2, next_attempt_at = $3, last_error = $4 WHERE event_id = $1")
		.bind(event_id)
		.bind(i64::from(tries))
		.bind(to_db(next))
		.bind(error)
		.execute(&mut *conn)
		.await
		.wrap_err("rescheduling an event for PostHog")?;
	Ok(())
}

/// How many events are owed.
pub async fn len(conn: &mut SqliteConnection) -> eyre::Result<u64> {
	let n: i64 = sqlx::query_scalar("SELECT count(*) FROM posthog_outbox")
		.fetch_one(&mut *conn)
		.await
		.wrap_err("counting the PostHog outbox")?;
	Ok(u64::try_from(n).unwrap_or(0))
}
