//! Postgres: runtime `sqlx` queries, migrations embedded from `migrations/`. Every query
//! here is executed by the integration tests (`tests/`), which stand in for the
//! compile-time checks the `query!` macros would give.

pub mod events;
pub mod metrics;
pub mod projections;
pub mod reads;
pub mod sessions;
pub mod sources;
pub mod telegram;

use chrono::{DateTime, NaiveDate, Utc};
use eyre::WrapErr;
use jiff::{Timestamp, civil::Date};
use sqlx::{
	Connection, Executor, PgConnection, PgPool,
	migrate::Migrator,
	postgres::{PgConnectOptions, PgPoolOptions},
};

/// The schema, embedded from `migrations/`.
static MIGRATOR: Migrator = sqlx::migrate!("./migrations");

/// The runtime role's grants, written for a role named `panel_app`.
const GRANTS: &str = include_str!("../../../../deploy/panel_app.sql");

/// The panel's database.
#[derive(Clone, Debug)]
pub struct Store {
	pool: PgPool,
}

impl Store {
	/// Applies the migrations `migrations/` holds and this database lacks, as the role that
	/// owns the schema (`panel migrate`, `MIGRATE_DATABASE_URL`). Nothing else migrates: the
	/// runtime role may not, so a rollout that ships a migration runs this first.
	pub async fn migrate(options: PgConnectOptions) -> eyre::Result<()> {
		let pool = PgPoolOptions::new()
			.max_connections(1)
			.connect_with(options)
			.await
			.wrap_err("connecting to Postgres to migrate")?;
		MIGRATOR.run(&pool).await.wrap_err("applying migrations")?;
		pool.close().await;
		Ok(())
	}

	/// Grants `role` what the runtime needs (`deploy/panel_app.sql`), as the schema's owner,
	/// in one transaction. Every grant is idempotent, so it runs after every migration: a
	/// new table is granted in the same step that made it. The role itself is the deploy's.
	pub async fn grant_runtime(options: PgConnectOptions, role: &str) -> eyre::Result<()> {
		// A role name cannot be a bind parameter: it is spliced, so only a plain one is taken.
		let plain = (1..=63).contains(&role.len())
			&& role.starts_with(|c: char| c.is_ascii_lowercase() || c == '_')
			&& role.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_');
		eyre::ensure!(plain, "the runtime role is a plain lowercase name, like panel_app: {role:?}");
		let mut conn = PgConnection::connect_with(&options).await.wrap_err("connecting to Postgres to grant")?;
		let mut tx = conn.begin().await.wrap_err("beginning the grants")?;
		tx.execute(sqlx::AssertSqlSafe(GRANTS.replace("panel_app", role)))
			.await
			.wrap_err_with(|| format!("granting {role} the runtime's rights (does the role exist?)"))?;
		tx.commit().await.wrap_err("committing the grants")?;
		conn.close().await.wrap_err("closing the grants' connection")
	}

	/// Connects as the runtime role, and refuses a database that lacks any of this build's
	/// migrations: its queries would fail one by one, later and less clearly.
	pub async fn connect(url: &str) -> eyre::Result<Self> {
		Self::connect_with(url.parse().wrap_err("DATABASE_URL is not a Postgres URL")?).await
	}

	pub async fn connect_with(options: PgConnectOptions) -> eyre::Result<Self> {
		// A request waits at most this long for a connection, then fails: a pool drained by a
		// burst answers 500 quickly instead of stacking every request behind it. Eight per pod:
		// the runtime role is capped at 20 connections (devops app-databases.nix), and a
		// rolling update runs two pods.
		let pool = PgPoolOptions::new()
			.max_connections(8)
			.acquire_timeout(std::time::Duration::from_secs(3))
			.connect_with(options)
			.await
			.wrap_err("connecting to Postgres")?;
		let store = Self { pool };
		store.check_schema().await?;
		Ok(store)
	}

	async fn check_schema(&self) -> eyre::Result<()> {
		let applied: Vec<i64> = sqlx::query_scalar("SELECT version FROM _sqlx_migrations WHERE success")
			.fetch_all(&self.pool)
			.await
			.wrap_err("reading which migrations are applied (none? run `panel migrate`)")?;
		let missing: Vec<String> = MIGRATOR
			.iter()
			.filter(|m| m.migration_type.is_up_migration() && !applied.contains(&m.version))
			.map(|m| format!("{} {}", m.version, m.description))
			.collect();
		// Newer ones than this build knows are fine: that is a rollback onto a schema moved on.
		eyre::ensure!(missing.is_empty(), "the database lacks migrations {}: run `panel migrate`", missing.join(", "));
		Ok(())
	}

	pub fn pool(&self) -> &PgPool {
		&self.pool
	}
}

pub(crate) fn to_pg(ts: Timestamp) -> eyre::Result<DateTime<Utc>> {
	// `subsec_nanosecond` is negative only before 1970, which `DateTime` takes as a
	// negative second and a positive fraction.
	let nanos = u32::try_from(ts.subsec_nanosecond().rem_euclid(1_000_000_000)).wrap_err("nanoseconds")?;
	let secs = ts.as_second() - i64::from(ts.subsec_nanosecond() < 0);
	DateTime::from_timestamp(secs, nanos).ok_or_else(|| eyre::eyre!("timestamp {ts} is out of Postgres' range"))
}

/// A civil date as the database's.
pub(crate) fn to_pg_day(d: Date) -> eyre::Result<NaiveDate> {
	NaiveDate::from_ymd_opt(i32::from(d.year()), u32::from(d.month().unsigned_abs()), u32::from(d.day().unsigned_abs())).ok_or_else(|| eyre::eyre!("date {d}"))
}

pub(crate) fn from_pg_day(d: NaiveDate) -> eyre::Result<Date> {
	use chrono::Datelike;
	let small = |v: u32| i8::try_from(v).wrap_err_with(|| format!("stored date {d}"));
	Date::new(i16::try_from(d.year()).wrap_err_with(|| format!("stored date {d}"))?, small(d.month())?, small(d.day())?).wrap_err_with(|| format!("stored date {d}"))
}

pub(crate) fn from_pg(ts: DateTime<Utc>) -> eyre::Result<Timestamp> {
	Timestamp::new(ts.timestamp(), i32::try_from(ts.timestamp_subsec_nanos()).wrap_err("nanoseconds")?).wrap_err_with(|| format!("stored timestamp {ts}"))
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn timestamps_round_trip() {
		for s in ["2026-09-30T10:00:00.123456Z", "1969-12-31T23:59:59.5Z", "1970-01-01T00:00:00Z"] {
			let ts: Timestamp = s.parse().unwrap();
			assert_eq!(from_pg(to_pg(ts).unwrap()).unwrap(), ts, "{s}");
		}
	}

	#[test]
	fn days_round_trip() {
		for s in ["2026-09-30", "2024-02-29", "0001-01-01"] {
			let d: Date = s.parse().unwrap();
			assert_eq!(from_pg_day(to_pg_day(d).unwrap()).unwrap(), d, "{s}");
		}
	}
}
