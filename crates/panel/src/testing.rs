//! For tests only (feature `testing`): a throwaway database per test, and signed batches.
//!
//! Each test gets its own SQLite file in the system's temp directory, removed when the
//! [`TestDb`] is dropped: no server, nothing to set up, so the database tests run everywhere
//! `cargo test` does, CI included. A file, not `:memory:`: an in-memory database is one per
//! connection, and the tests open several pools on one database, as replicas would.

use std::path::{Path, PathBuf};

use jiff::Timestamp;
use panel_core::signature;
use serde_json::{Value, json};
use sqlx::sqlite::SqlitePoolOptions;
use uuid::Uuid;

use crate::{
	Panel,
	seal::DataKey,
	store::{self, Store},
};

/// A database that lives as long as this value.
pub struct TestDb {
	path: PathBuf,
}

impl TestDb {
	/// A fresh, empty database.
	pub async fn create() -> Self {
		let path = std::env::temp_dir().join(format!("panel_test_{}.db", Uuid::now_v7().simple()));
		Self { path }
	}

	pub fn path(&self) -> &Path {
		&self.path
	}

	/// A store on this database, migrated (opening migrates).
	pub async fn store(&self) -> Store {
		Store::open(&self.path).await.expect("opening the test database")
	}

	/// A plain pool on this database, for assertions the engine has no API for. Migrated
	/// first, so it can be asked for before any store.
	pub async fn pool(&self) -> sqlx::SqlitePool {
		drop(self.store().await);
		SqlitePoolOptions::new()
			.max_connections(2)
			.connect_with(store::options(&self.path))
			.await
			.expect("connecting to the test database")
	}
}

impl Drop for TestDb {
	fn drop(&mut self) {
		// The WAL and its index sit beside the file; a pool still open only keeps them a while.
		for suffix in ["", "-wal", "-shm"] {
			let mut path = self.path.clone().into_os_string();
			path.push(suffix);
			match std::fs::remove_file(&path) {
				Ok(()) => {}
				Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
				Err(e) => eprintln!("could not remove test database {}: {e}", PathBuf::from(path).display()),
			}
		}
	}
}

/// A panel on a fresh store with a fresh data key.
pub async fn panel(db: &TestDb) -> Panel {
	let key = DataKey::from_hex(&DataKey::generate_hex().expect("random")).expect("a fresh key");
	Panel::new(db.store().await, key)
}

/// A request as a source sends it: the body, and the headers' values.
pub struct Signed {
	pub key_id: String,
	pub timestamp: String,
	pub signature: String,
	pub body: Vec<u8>,
}

impl Signed {
	pub fn batch(&self) -> crate::SignedBatch<'_> {
		crate::SignedBatch {
			key_id: &self.key_id,
			timestamp: &self.timestamp,
			signature: &self.signature,
			body: &self.body,
		}
	}
}

/// `{"events": events}`, signed at `at` with `secret`, as a source sends it: each event's
/// `source.id` set to the key id, which is what a source is called.
pub fn sign(key_id: &str, secret: &str, events: &[Value], at: Timestamp) -> Signed {
	let events: Vec<Value> = events
		.iter()
		.cloned()
		.map(|mut e| {
			if let Some(source) = e.get_mut("source").and_then(Value::as_object_mut) {
				source.insert("id".to_owned(), json!(key_id));
			}
			e
		})
		.collect();
	sign_verbatim(key_id, secret, &events, at)
}

/// [`sign`], leaving the events exactly as given.
pub fn sign_verbatim(key_id: &str, secret: &str, events: &[Value], at: Timestamp) -> Signed {
	let body = serde_json::to_vec(&json!({ "events": events })).expect("JSON");
	let timestamp = at.as_second().to_string();
	Signed {
		key_id: key_id.to_owned(),
		signature: signature::sign(secret.as_bytes(), &timestamp, &body),
		timestamp,
		body,
	}
}

/// An event as a source writes it, with a fresh id.
pub fn event(r#type: &str, occurred_at: Timestamp, kind: &str, subject: Value, properties: Value) -> Value {
	json!({
		"id": Uuid::now_v7().to_string(),
		"schema": "sa.funnel.v1",
		"type": r#type,
		"typeVersion": 1,
		"occurredAt": occurred_at.to_string(),
		"source": {"kind": kind, "id": "test"},
		"subject": subject,
		"properties": properties,
	})
}

/// What `sa:operator` holds.
pub fn operator() -> sa_auth::PermissionSet {
	sa_auth::SA_OPERATOR.members.iter().copied().collect()
}

/// What `sa:admin` holds.
pub fn admin() -> sa_auth::PermissionSet {
	sa_auth::SA_ADMIN.members.iter().copied().collect()
}
