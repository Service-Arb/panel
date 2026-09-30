//! The runtime role's grants (`deploy/panel_app.sql`) are enough for everything the runtime
//! does, and no more: the schema is migrated by its owner, then the engine runs as a role
//! holding only those grants.

use jiff::{SignedDuration, Timestamp};
use panel::{
	Outcome, Panel,
	seal::DataKey,
	store::Store,
	testing::{TestDb, event, sign},
};
use panel_core::{event::SourceKind, ids::BrandId};
use serde_json::json;
use sqlx::{Connection, Executor, PgConnection};

const GRANTS: &str = include_str!("../../../deploy/panel_app.sql");

/// A login role for this test alone, holding the runtime grants, dropped at the end.
struct AppRole {
	admin: sqlx::postgres::PgConnectOptions,
	name: String,
}

impl AppRole {
	async fn create(db: &TestDb) -> Self {
		let name = format!("panel_app_test_{}", uuid::Uuid::now_v7().simple());
		let mut owner = PgConnection::connect_with(&db.options).await.unwrap();
		// The name is ours (a UUID's hex): safe to splice into the statements.
		owner.execute(sqlx::AssertSqlSafe(format!("CREATE ROLE {name} LOGIN"))).await.unwrap();
		owner.execute(sqlx::AssertSqlSafe(GRANTS.replace("panel_app", &name))).await.unwrap();
		owner.close().await.unwrap();
		Self { admin: db.options.clone(), name }
	}

	fn options(&self, db: &TestDb) -> sqlx::postgres::PgConnectOptions {
		db.options.clone().username(&self.name)
	}
}

impl Drop for AppRole {
	fn drop(&mut self) {
		// Roles are the server's, not the test database's: drop it even when the test failed.
		// Drop runs outside any async context, hence a runtime of its own on a thread.
		let (admin, name) = (self.admin.clone(), self.name.clone());
		let dropped = std::thread::spawn(move || {
			tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap().block_on(async move {
				let mut owner = PgConnection::connect_with(&admin).await?;
				owner.execute(sqlx::AssertSqlSafe(format!("DROP OWNED BY {name}; DROP ROLE {name}"))).await?;
				owner.close().await
			})
		})
		.join();
		if !matches!(dropped, Ok(Ok(()))) {
			eprintln!("could not drop test role {}: {dropped:?}", self.name);
		}
	}
}

#[tokio::test]
async fn the_runtime_role_does_its_work_and_nothing_else() {
	let Some(db) = TestDb::create().await else { return };
	let fresh = Store::connect_with(db.options.clone()).await.unwrap_err();
	assert!(format!("{fresh:#}").contains("panel migrate"), "{fresh:#}");
	Store::migrate(db.options.clone()).await.unwrap();
	Store::migrate(db.options.clone()).await.expect("again: nothing to do");

	let role = AppRole::create(&db).await;
	{
		let key = DataKey::from_hex(&DataKey::generate_hex().unwrap()).unwrap();
		let panel = Panel::new(Store::connect_with(role.options(&db)).await.unwrap(), key);
		let secret = panel
			.add_source("aquafix-ops", SourceKind::Panel, [BrandId::parse("aquafix").unwrap()].into())
			.await
			.unwrap()
			.unwrap()
			.secret
			.to_string();
		let now = Timestamp::now();
		let lead = json!({"brandId": "aquafix", "leadId": "L-1"});
		let events = [
			event("lead.created", now - SignedDuration::from_mins(5), "panel", lead.clone(), json!({"channel": "phone_inbound"})),
			event("call.logged", now, "panel", lead.clone(), json!({"outcome": "answered"})),
			event("payment.received", now, "panel", lead, json!({"billed": 100, "commission": 10, "currency": "EUR"})),
		];
		let got = panel.ingest(sign("aquafix-ops", &secret, &events, now).batch(), now).await.unwrap();
		assert!(got.iter().all(|v| v.outcome == Outcome::Accepted { unregistered: false }), "{got:?}");
		assert_eq!(panel.rebuild_projections().await.unwrap().leads, 1);
		assert!(panel.store().revoke_source("aquafix-ops").await.unwrap());

		let pool = panel.store().pool();
		for sql in [
			"DELETE FROM events",
			"UPDATE events SET brand_id = 'x'",
			"DELETE FROM sources",
			"UPDATE sources SET brand_ids = '{x}'",
			"CREATE TABLE t (x int)",
			"DROP VIEW reporting.leads",
		] {
			let err = sqlx::query(sqlx::AssertSqlSafe(sql)).execute(pool).await.unwrap_err().to_string();
			assert!(err.contains("permission denied") || err.contains("must be owner"), "{sql}: {err}");
		}
		let n: i64 = sqlx::query_scalar("SELECT count(*) FROM reporting.leads").fetch_one(pool).await.unwrap();
		assert_eq!(n, 1);
		pool.close().await;
	}
}
