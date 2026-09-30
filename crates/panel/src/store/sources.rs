//! The sources that may write events: a key id, the kind of source holding it, the brands
//! it may write for, and its sealed HMAC secret.

use std::collections::BTreeSet;

use chrono::{DateTime, Utc};
use eyre::WrapErr;
use panel_core::{
	event::{KeyGrant, SourceKind},
	ids::BrandId,
};

use super::Store;

/// A source's row, secret still sealed.
#[derive(Clone, Debug)]
pub struct SourceRow {
	pub grant: KeyGrant,
	pub secret_sealed: Vec<u8>,
	pub data_key_fp: Vec<u8>,
	pub created_at: DateTime<Utc>,
	pub revoked_at: Option<DateTime<Utc>>,
}

#[derive(sqlx::FromRow)]
struct Row {
	key_id: String,
	kind: String,
	brand_ids: Vec<String>,
	secret_sealed: Vec<u8>,
	data_key_fp: Vec<u8>,
	created_at: DateTime<Utc>,
	revoked_at: Option<DateTime<Utc>>,
}

impl TryFrom<Row> for SourceRow {
	type Error = eyre::Report;

	fn try_from(r: Row) -> eyre::Result<Self> {
		let kind: SourceKind = r.kind.parse().wrap_err_with(|| format!("source {}", r.key_id))?;
		let brands = r
			.brand_ids
			.iter()
			.map(|b| BrandId::parse(b))
			.collect::<Result<BTreeSet<_>, _>>()
			.wrap_err_with(|| format!("source {}", r.key_id))?;
		Ok(Self {
			grant: KeyGrant { key_id: r.key_id, kind, brands },
			secret_sealed: r.secret_sealed,
			data_key_fp: r.data_key_fp,
			created_at: r.created_at,
			revoked_at: r.revoked_at,
		})
	}
}

// A macro, not a const, so every query stays a literal (`concat!`) that sqlx takes as audited.
macro_rules! columns {
	() => {
		"key_id, kind, brand_ids, secret_sealed, data_key_fp, created_at, revoked_at"
	};
}

impl Store {
	/// Adds a source; `false` when the key id is taken.
	pub async fn insert_source(&self, grant: &KeyGrant, secret_sealed: &[u8], data_key_fp: &[u8]) -> eyre::Result<bool> {
		let brands: Vec<&str> = grant.brands.iter().map(BrandId::as_str).collect();
		let inserted = sqlx::query("INSERT INTO sources (key_id, kind, brand_ids, secret_sealed, data_key_fp) VALUES ($1, $2, $3, $4, $5) ON CONFLICT (key_id) DO NOTHING")
			.bind(&grant.key_id)
			.bind(grant.kind.as_str())
			.bind(&brands)
			.bind(secret_sealed)
			.bind(data_key_fp)
			.execute(&self.pool)
			.await
			.wrap_err("inserting a source")?
			.rows_affected();
		Ok(inserted == 1)
	}

	/// A source that may still write: not revoked.
	pub async fn active_source(&self, key_id: &str) -> eyre::Result<Option<SourceRow>> {
		sqlx::query_as::<_, Row>(concat!("SELECT ", columns!(), " FROM sources WHERE key_id = $1 AND revoked_at IS NULL"))
			.bind(key_id)
			.fetch_optional(&self.pool)
			.await
			.wrap_err("reading a source")?
			.map(SourceRow::try_from)
			.transpose()
	}

	pub async fn sources(&self) -> eyre::Result<Vec<SourceRow>> {
		sqlx::query_as::<_, Row>(concat!("SELECT ", columns!(), " FROM sources ORDER BY key_id"))
			.fetch_all(&self.pool)
			.await
			.wrap_err("listing sources")?
			.into_iter()
			.map(SourceRow::try_from)
			.collect()
	}

	/// Revokes a source; `false` when there is no active one by that id. Its events stay.
	pub async fn revoke_source(&self, key_id: &str) -> eyre::Result<bool> {
		let revoked = sqlx::query("UPDATE sources SET revoked_at = now() WHERE key_id = $1 AND revoked_at IS NULL")
			.bind(key_id)
			.execute(&self.pool)
			.await
			.wrap_err("revoking a source")?
			.rows_affected();
		Ok(revoked == 1)
	}
}
