//! Sign-in sessions: the key of each is the hash of its cookie, and its concierge tokens are
//! sealed.

use chrono::{DateTime, Utc};
use eyre::WrapErr;
use jiff::Timestamp;
use sqlx::PgConnection;
use uuid::Uuid;

use super::{from_pg, to_pg};

/// A session's row, tokens still sealed.
#[derive(Clone, Debug)]
pub struct SessionRow {
	pub user_id: Uuid,
	pub access_sealed: Vec<u8>,
	pub access_expires_at: Timestamp,
	pub refresh_sealed: Vec<u8>,
	pub data_key_fp: Vec<u8>,
	pub expires_at: Timestamp,
}

#[derive(sqlx::FromRow)]
struct Row {
	user_id: Uuid,
	access_sealed: Vec<u8>,
	access_expires_at: DateTime<Utc>,
	refresh_sealed: Vec<u8>,
	data_key_fp: Vec<u8>,
	expires_at: DateTime<Utc>,
}

impl TryFrom<Row> for SessionRow {
	type Error = eyre::Report;

	fn try_from(r: Row) -> eyre::Result<Self> {
		Ok(Self {
			user_id: r.user_id,
			access_sealed: r.access_sealed,
			access_expires_at: from_pg(r.access_expires_at)?,
			refresh_sealed: r.refresh_sealed,
			data_key_fp: r.data_key_fp,
			expires_at: from_pg(r.expires_at)?,
		})
	}
}

/// The sealed tokens of a session, as written.
pub struct SealedTokens<'a> {
	pub access: &'a [u8],
	pub access_expires_at: Timestamp,
	pub refresh: &'a [u8],
	pub data_key_fp: [u8; 32],
	pub expires_at: Timestamp,
}

pub async fn insert(conn: &mut PgConnection, id_hash: &[u8], user_id: Uuid, t: &SealedTokens<'_>) -> eyre::Result<()> {
	sqlx::query(
		"INSERT INTO sessions (id_hash, user_id, access_sealed, access_expires_at, refresh_sealed, data_key_fp, expires_at) \
		 VALUES ($1, $2, $3, $4, $5, $6, $7)",
	)
	.bind(id_hash)
	.bind(user_id)
	.bind(t.access)
	.bind(to_pg(t.access_expires_at)?)
	.bind(t.refresh)
	.bind(t.data_key_fp.as_slice())
	.bind(to_pg(t.expires_at)?)
	.execute(&mut *conn)
	.await
	.wrap_err("opening a session")?;
	Ok(())
}

// A macro, not a const, so every query stays a literal (`concat!`) that sqlx takes as audited.
macro_rules! columns {
	() => {
		"user_id, access_sealed, access_expires_at, refresh_sealed, data_key_fp, expires_at"
	};
}

pub async fn get(conn: &mut PgConnection, id_hash: &[u8]) -> eyre::Result<Option<SessionRow>> {
	sqlx::query_as::<_, Row>(concat!("SELECT ", columns!(), " FROM sessions WHERE id_hash = $1"))
		.bind(id_hash)
		.fetch_optional(&mut *conn)
		.await
		.wrap_err("reading a session")?
		.map(SessionRow::try_from)
		.transpose()
}

/// Takes the lease on rotating a session whose access token still expires at `seen`: `true`
/// for the one caller that gets it, until `until`. A lease past its time is taken over — its
/// holder died or hung.
pub async fn claim_rotation(conn: &mut PgConnection, id_hash: &[u8], seen: Timestamp, now: Timestamp, until: Timestamp) -> eyre::Result<bool> {
	let claimed = sqlx::query(
		"UPDATE sessions SET rotating_until = $4 \
		 WHERE id_hash = $1 AND access_expires_at = $2 AND (rotating_until IS NULL OR rotating_until <= $3)",
	)
	.bind(id_hash)
	.bind(to_pg(seen)?)
	.bind(to_pg(now)?)
	.bind(to_pg(until)?)
	.execute(&mut *conn)
	.await
	.wrap_err("leasing a session's rotation")?
	.rows_affected();
	Ok(claimed == 1)
}

/// Writes the rotated tokens and drops the lease, if the row still has the tokens the
/// rotation started from (`seen`). `false`: it does not.
pub async fn set_tokens(conn: &mut PgConnection, id_hash: &[u8], seen: Timestamp, t: &SealedTokens<'_>) -> eyre::Result<bool> {
	let set = sqlx::query(
		"UPDATE sessions SET access_sealed = $3, access_expires_at = $4, refresh_sealed = $5, data_key_fp = $6, expires_at = $7, \
		 rotating_until = NULL WHERE id_hash = $1 AND access_expires_at = $2",
	)
	.bind(id_hash)
	.bind(to_pg(seen)?)
	.bind(t.access)
	.bind(to_pg(t.access_expires_at)?)
	.bind(t.refresh)
	.bind(t.data_key_fp.as_slice())
	.bind(to_pg(t.expires_at)?)
	.execute(&mut *conn)
	.await
	.wrap_err("rotating a session's tokens")?
	.rows_affected();
	Ok(set == 1)
}

/// Gives the lease back without rotating: concierge could not be asked.
pub async fn release_rotation(conn: &mut PgConnection, id_hash: &[u8], seen: Timestamp) -> eyre::Result<()> {
	sqlx::query("UPDATE sessions SET rotating_until = NULL WHERE id_hash = $1 AND access_expires_at = $2")
		.bind(id_hash)
		.bind(to_pg(seen)?)
		.execute(&mut *conn)
		.await
		.wrap_err("releasing a session's rotation")?;
	Ok(())
}

/// Closes the session if it still has the tokens concierge just refused (`seen`).
pub async fn delete_refused(conn: &mut PgConnection, id_hash: &[u8], seen: Timestamp) -> eyre::Result<()> {
	sqlx::query("DELETE FROM sessions WHERE id_hash = $1 AND access_expires_at = $2")
		.bind(id_hash)
		.bind(to_pg(seen)?)
		.execute(&mut *conn)
		.await
		.wrap_err("closing a refused session")?;
	Ok(())
}

/// Closes every session of the user the session `id_hash` belongs to; whose they were, or
/// `None` when there was no such session.
pub async fn delete_all_of(conn: &mut PgConnection, id_hash: &[u8]) -> eyre::Result<Option<Uuid>> {
	let user: Option<Uuid> = sqlx::query_scalar(
		"WITH owner AS (SELECT user_id FROM sessions WHERE id_hash = $1), \
		 gone AS (DELETE FROM sessions WHERE user_id IN (SELECT user_id FROM owner)) \
		 SELECT user_id FROM owner",
	)
	.bind(id_hash)
	.fetch_optional(&mut *conn)
	.await
	.wrap_err("closing a user's sessions")?;
	Ok(user)
}

/// Marks a callback's state as redeemed; `false` when it was already. Expired marks are
/// dropped on the way, so the table holds only the last [`crate::session::PRELOGIN_TTL`].
pub async fn consume_state(conn: &mut PgConnection, state_hash: &[u8], now: Timestamp, expires_at: Timestamp) -> eyre::Result<bool> {
	sqlx::query("DELETE FROM consumed_states WHERE expires_at <= $1")
		.bind(to_pg(now)?)
		.execute(&mut *conn)
		.await
		.wrap_err("pruning consumed states")?;
	let inserted = sqlx::query("INSERT INTO consumed_states (state_hash, expires_at) VALUES ($1, $2) ON CONFLICT (state_hash) DO NOTHING")
		.bind(state_hash)
		.bind(to_pg(expires_at)?)
		.execute(&mut *conn)
		.await
		.wrap_err("consuming a sign-in state")?
		.rows_affected();
	Ok(inserted == 1)
}

/// `false` when there was no such session.
pub async fn delete(conn: &mut PgConnection, id_hash: &[u8]) -> eyre::Result<bool> {
	let deleted = sqlx::query("DELETE FROM sessions WHERE id_hash = $1")
		.bind(id_hash)
		.execute(&mut *conn)
		.await
		.wrap_err("closing a session")?
		.rows_affected();
	Ok(deleted == 1)
}

/// The id hash of the user's newest session still within its deadline.
pub async fn newest_of_user(conn: &mut PgConnection, user_id: Uuid, now: Timestamp) -> eyre::Result<Option<Vec<u8>>> {
	sqlx::query_scalar("SELECT id_hash FROM sessions WHERE user_id = $1 AND expires_at > $2 ORDER BY created_at DESC LIMIT 1")
		.bind(user_id)
		.bind(to_pg(now)?)
		.fetch_optional(&mut *conn)
		.await
		.wrap_err("finding a user's session")
}

/// Drops the sessions past their deadline.
pub async fn prune(conn: &mut PgConnection, now: Timestamp) -> eyre::Result<u64> {
	Ok(sqlx::query("DELETE FROM sessions WHERE expires_at <= $1")
		.bind(to_pg(now)?)
		.execute(&mut *conn)
		.await
		.wrap_err("pruning expired sessions")?
		.rows_affected())
}
