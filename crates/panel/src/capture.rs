//! Sending PostHog what it is owed (`store::posthog`): the lead events queued as they were
//! journaled, in batches, through the [`Capturer`] port (the server's HTTP client).
//!
//! ```text
//! queue        Panel::journal, in the event's own transaction, when capture is on: one row per
//!              new lead event panel_core::analytics names; never the rebuild
//! pass (5 s)   ≤ 100 due rows → one batch → PostHog took it: deleted
//!              retry (5xx, 429, no answer)  → each row again in 10 s, doubling to 15 min
//!              refused (another 4xx)        → a batch of several is tried row by row; a row
//!                                             refused alone is dropped, and said so
//!              a row queued 7 days ago and still failing is dropped, and said so
//! ```

use eyre::WrapErr;
use jiff::{SignedDuration, Timestamp};
use serde_json::Value;
use uuid::Uuid;

use crate::{
	Panel,
	store::posthog::{self as outbox, Queued},
};

/// Rows sent at once.
pub const BATCH: i64 = 100;
/// The first wait after a failure; it doubles up to [`MAX_BACKOFF`].
pub const FIRST_BACKOFF: SignedDuration = SignedDuration::from_secs(10);
pub const MAX_BACKOFF: SignedDuration = SignedDuration::from_mins(15);
/// A row still failing this long after it was queued is given up.
pub const GIVE_UP_AFTER: SignedDuration = SignedDuration::from_hours(24 * 7);

/// One event as PostHog's batch endpoint takes it.
#[derive(Clone, Debug, PartialEq)]
pub struct Captured {
	/// The journal's event id: PostHog deduplicates by it.
	pub uuid: Uuid,
	pub event: String,
	pub distinct_id: String,
	pub properties: Value,
	/// When it happened, not when it is sent.
	pub timestamp: Timestamp,
}

/// Why a batch did not go.
#[derive(Debug, thiserror::Error)]
pub enum SendError {
	/// Worth trying again: no answer, a 5xx, a 429.
	#[error("PostHog did not take it, for now: {0}")]
	Retry(String),
	/// PostHog will never take it as it is: another 4xx.
	#[error("PostHog refused it: {0}")]
	Refused(String),
}

/// Where the events go: PostHog's capture API.
pub trait Capturer {
	fn send(&self, batch: &[Captured]) -> impl Future<Output = Result<(), SendError>> + Send;
}

/// What a pass did.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Delivered {
	pub sent: u64,
	pub retried: u64,
	pub dropped: u64,
}

/// The wait before try `tries + 1`.
pub fn backoff(tries: u32) -> SignedDuration {
	let mut wait = FIRST_BACKOFF;
	for _ in 1..tries.min(20) {
		wait = (wait * 2).min(MAX_BACKOFF);
	}
	wait.min(MAX_BACKOFF)
}

fn captured(q: &Queued) -> Captured {
	Captured {
		uuid: q.event_id,
		event: q.event.clone(),
		distinct_id: q.distinct_id.clone(),
		properties: q.properties.clone(),
		timestamp: q.occurred_at,
	}
}

impl Panel {
	/// Queues PostHog's events from now on; off by default, and in every command but `serve`
	/// with a project key.
	pub fn with_capture(mut self, on: bool) -> Self {
		self.capture = on;
		self
	}

	/// How many events PostHog is owed.
	pub async fn capture_backlog(&self) -> eyre::Result<u64> {
		let mut conn = self.store.pool().acquire().await.wrap_err("a connection")?;
		outbox::len(&mut conn).await
	}

	/// Sends one batch of what is due at `now`.
	pub async fn capture_pass(&self, to: &impl Capturer, now: Timestamp) -> eyre::Result<Delivered> {
		let due = {
			let mut conn = self.store.pool().acquire().await.wrap_err("a connection")?;
			outbox::due(&mut conn, now, BATCH).await?
		};
		let mut done = Delivered::default();
		if due.is_empty() {
			return Ok(done);
		}
		let batch: Vec<Captured> = due.iter().map(captured).collect();
		match to.send(&batch).await {
			Ok(()) => self.sent(&due, &mut done).await?,
			Err(SendError::Retry(why)) => self.failed(&due, &why, now, &mut done).await?,
			Err(SendError::Refused(why)) if due.len() == 1 => self.refused(&due[0], &why, &mut done).await?,
			// One bad row must not hold back the others: each alone, to find it.
			Err(SendError::Refused(_)) =>
				for q in &due {
					match to.send(&[captured(q)]).await {
						Ok(()) => self.sent(std::slice::from_ref(q), &mut done).await?,
						Err(SendError::Retry(why)) => self.failed(std::slice::from_ref(q), &why, now, &mut done).await?,
						Err(SendError::Refused(why)) => self.refused(q, &why, &mut done).await?,
					}
				},
		}
		Ok(done)
	}

	async fn sent(&self, rows: &[Queued], done: &mut Delivered) -> eyre::Result<()> {
		let mut tx = self.store.begin_write().await?;
		for q in rows {
			outbox::remove(&mut tx, q.event_id).await?;
		}
		tx.commit().await.wrap_err("committing what PostHog took")?;
		done.sent += rows.len() as u64;
		Ok(())
	}

	async fn failed(&self, rows: &[Queued], why: &str, now: Timestamp, done: &mut Delivered) -> eyre::Result<()> {
		let why: String = why.chars().take(300).collect();
		let mut tx = self.store.begin_write().await?;
		for q in rows {
			if now.duration_since(q.queued_at) >= GIVE_UP_AFTER {
				tracing::warn!(event_id = %q.event_id, event = q.event, error = why, "posthog: given up on an event after a week of failures");
				outbox::remove(&mut tx, q.event_id).await?;
				done.dropped += 1;
			} else {
				let tries = q.tries.saturating_add(1);
				outbox::retry(&mut tx, q.event_id, tries, now + backoff(tries), &why).await?;
				done.retried += 1;
			}
		}
		tx.commit().await.wrap_err("rescheduling what PostHog did not take")?;
		Ok(())
	}

	async fn refused(&self, q: &Queued, why: &str, done: &mut Delivered) -> eyre::Result<()> {
		tracing::warn!(event_id = %q.event_id, event = q.event, error = why, "posthog: refused an event; dropped");
		let mut conn = self.store.pool().acquire().await.wrap_err("a connection")?;
		outbox::remove(&mut conn, q.event_id).await?;
		done.dropped += 1;
		Ok(())
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn the_backoff_doubles_to_its_cap() {
		assert_eq!(backoff(1), SignedDuration::from_secs(10));
		assert_eq!(backoff(2), SignedDuration::from_secs(20));
		assert_eq!(backoff(4), SignedDuration::from_secs(80));
		assert_eq!(backoff(10), MAX_BACKOFF);
		assert_eq!(backoff(u32::MAX), MAX_BACKOFF);
	}
}
