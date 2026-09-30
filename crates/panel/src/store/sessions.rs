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

/// The row, locked until the transaction ends: whoever holds it is the one refreshing.
pub async fn lock(conn: &mut PgConnection, id_hash: &[u8]) -> eyre::Result<Option<SessionRow>> {
	sqlx::query_as::<_, Row>(concat!("SELECT ", columns!(), " FROM sessions WHERE id_hash = $1 FOR UPDATE"))
		.bind(id_hash)
		.fetch_optional(&mut *conn)
		.await
		.wrap_err("locking a session")?
		.map(SessionRow::try_from)
		.transpose()
}

pub async fn set_tokens(conn: &mut PgConnection, id_hash: &[u8], t: &SealedTokens<'_>) -> eyre::Result<()> {
	sqlx::query("UPDATE sessions SET access_sealed = $2, access_expires_at = $3, refresh_sealed = $4, data_key_fp = $5, expires_at = $6 WHERE id_hash = $1")
		.bind(id_hash)
		.bind(t.access)
		.bind(to_pg(t.access_expires_at)?)
		.bind(t.refresh)
		.bind(t.data_key_fp.as_slice())
		.bind(to_pg(t.expires_at)?)
		.execute(&mut *conn)
		.await
		.wrap_err("rotating a session's tokens")?;
	Ok(())
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

/// Drops the sessions past their deadline.
pub async fn prune(conn: &mut PgConnection, now: Timestamp) -> eyre::Result<u64> {
	Ok(sqlx::query("DELETE FROM sessions WHERE expires_at <= $1")
		.bind(to_pg(now)?)
		.execute(&mut *conn)
		.await
		.wrap_err("pruning expired sessions")?
		.rows_affected())
}
