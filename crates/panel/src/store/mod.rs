//! Postgres: runtime `sqlx` queries, migrations embedded from `migrations/`. Every query
//! here is executed by the integration tests (`tests/`), which stand in for the
//! compile-time checks the `query!` macros would give.

pub mod events;
pub mod projections;
pub mod sources;

use chrono::{DateTime, Utc};
use eyre::WrapErr;
use jiff::Timestamp;
use sqlx::{
	PgPool,
	postgres::{PgConnectOptions, PgPoolOptions},
};

/// The panel's database.
#[derive(Clone, Debug)]
pub struct Store {
	pool: PgPool,
}

impl Store {
	/// Connects and brings the schema up to date. Migrations run here, at start: shipping
	/// an image with a new migration is applying it.
	pub async fn connect(url: &str) -> eyre::Result<Self> {
		Self::connect_with(url.parse().wrap_err("DATABASE_URL is not a Postgres URL")?).await
	}

	pub async fn connect_with(options: PgConnectOptions) -> eyre::Result<Self> {
		let pool = PgPoolOptions::new().max_connections(16).connect_with(options).await.wrap_err("connecting to Postgres")?;
		sqlx::migrate!("./migrations").run(&pool).await.wrap_err("applying migrations")?;
		Ok(Self { pool })
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
}
