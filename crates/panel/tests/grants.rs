//! The runtime role's grants (`deploy/panel_app.sql`) are enough for everything the runtime
//! does, and no more: the schema is migrated by its owner, then the engine runs as a role
//! holding only those grants.

use jiff::{SignedDuration, Timestamp};
use panel::{
	Outcome, Panel,
	operator::{Actor, FunnelBy, LeadQuery, NewLead, Payment, Pii},
	posthog::Hogql,
	seal::DataKey,
	session::{RefreshError, Refresher, SessionKey, Tokens},
	store::Store,
	telegram::{Account, Bot, Chat, Directory, DirectoryError, Identity, InlineButton, Notifier, Update},
	testing::{TestDb, event, sign},
};
use panel_core::{
	event::SourceKind,
	ids::{BrandId, LocationId},
	notify::{Failure, Locale, Rendered, Rule},
	role::Role,
};
use serde_json::{Value, json};
use sqlx::{Connection, Executor, PgConnection};
use zeroize::Zeroizing;

/// PostHog answering one visit, one intent and one exposure today.
struct OneOfEach;

impl Hogql for OneOfEach {
	async fn query(&self, hogql: &str, _: &Value) -> eyre::Result<Vec<Vec<Value>>> {
		let day = Timestamp::now().to_zoned(jiff::tz::TimeZone::UTC).date().to_string();
		Ok(vec![if hogql.contains("'location_page_view'") {
			vec![json!(day), json!("aquafix"), json!("paris-11"), json!("direct"), json!(1)]
		} else if hogql.contains("'contact_intent_click'") {
			vec![json!(day), json!("aquafix"), json!("paris-11"), json!("phone"), json!(1)]
		} else {
			vec![json!(day), json!("aquafix"), json!("hero"), json!("a"), json!("experiment_exposed"), Value::Null, json!(1)]
		}])
	}
}

/// A refresher that must not be asked: the tokens above are fresh.
struct Never;

impl Refresher for Never {
	async fn refresh(&self, _: &str) -> Result<Tokens, RefreshError> {
		unreachable!("the tokens are fresh")
	}
}

/// A refresher that rotates once, to `a2`/`r2`.
struct Rotates;

impl Refresher for Rotates {
	async fn refresh(&self, _: &str) -> Result<Tokens, RefreshError> {
		let now = Timestamp::now();
		Ok(Tokens {
			access: Zeroizing::new("a2".into()),
			access_expires_at: now + SignedDuration::from_hours(24 * 400),
			refresh: Zeroizing::new("r2".into()),
			refresh_expires_at: now + SignedDuration::from_hours(24 * 400),
		})
	}
}

/// A bot that delivers everything, or refuses every send as blocked.
struct FakeBot {
	blocked: bool,
}

impl Bot for FakeBot {
	async fn send(&self, _: i64, _: &Rendered, _: &[InlineButton]) -> Result<i64, Failure> {
		if self.blocked { Err(Failure::Blocked) } else { Ok(1) }
	}

	async fn edit(&self, _: i64, _: i64, _: &Rendered, _: &[InlineButton]) -> Result<(), Failure> {
		Ok(())
	}

	async fn answer(&self, _: &str, _: &str) -> Result<(), Failure> {
		Ok(())
	}
}

/// Refreshes nothing; says every token is an operator's.
struct Operators;

impl Refresher for Operators {
	async fn refresh(&self, _: &str) -> Result<Tokens, RefreshError> {
		Err(RefreshError::Rejected)
	}
}

impl Directory for Operators {
	async fn me(&self, _: &str) -> Result<Identity, DirectoryError> {
		Err(DirectoryError::Refused)
	}
}

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
		owner.close().await.unwrap();
		// What `panel migrate --grant-to` runs.
		Store::grant_runtime(db.options.clone(), &name).await.unwrap();
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

		// Signed-in users: a session, and what an operator does and reads.
		let tokens = Tokens {
			access: Zeroizing::new("a".into()),
			access_expires_at: now + SignedDuration::from_mins(15),
			refresh: Zeroizing::new("r".into()),
			refresh_expires_at: now + SignedDuration::from_hours(1),
		};
		let user = uuid::Uuid::now_v7();
		let opened = panel.open_session(user, &tokens, now).await.unwrap();
		assert_eq!(panel.session(&opened.cookie, now, &Never).await.unwrap().user_id, user);
		let by = Actor(user);
		let new_lead = || NewLead {
			brand: BrandId::parse("aquafix").unwrap(),
			location: LocationId::parse("paris-11").unwrap(),
			need: "a boiler".into(),
			phone: None,
		};
		let (lead, _) = panel.create_lead(by, new_lead(), now).await.unwrap();
		let brand = BrandId::parse("aquafix").unwrap();
		panel.attempt_call(by, &brand, &lead, now).await.unwrap();
		assert_eq!(panel.leads(&LeadQuery { limit: 10, ..LeadQuery::default() }, Pii::Reveal, now).await.unwrap().leads.len(), 2);
		assert!(panel.lead_card(&brand, &lead, Pii::Reveal, now).await.unwrap().is_some());
		let today = now.to_zoned(jiff::tz::TimeZone::UTC).date();
		assert_eq!(panel.funnel(today, today, None).await.unwrap().manual, 2);
		let payment = Payment {
			billed: 100,
			commission: 10,
			currency: "EUR".into(),
		};
		panel.record_payment(by, &brand, &lead, payment, now).await.unwrap();
		let slices = panel.funnel_slices(today, today, None, FunnelBy::Location).await.unwrap();
		assert_eq!(slices[0].payments[0].billed, 100);
		assert_eq!(panel.places().await.unwrap().len(), 1);
		assert_eq!(panel.lead_counts(Some(&brand), None, now).await.unwrap().overdue, 0);
		assert!(panel.close_session(&SessionKey::of_cookie(&opened.cookie).unwrap()).await.unwrap());

		// A rotation (its lease, its guarded write), a redeemed state, a sign-out everywhere.
		let opened = panel.open_session(user, &tokens, now).await.unwrap();
		let stale = now + SignedDuration::from_mins(15);
		assert_eq!(panel.session(&opened.cookie, stale, &Rotates).await.unwrap().access.as_str(), "a2");
		assert!(panel.consume_state("state", now).await.unwrap());
		assert!(!panel.consume_state("state", now).await.unwrap());
		assert!(panel.consume_state("state", now + SignedDuration::from_hours(1)).await.unwrap(), "expired marks are dropped");
		assert_eq!(panel.close_all_sessions(&SessionKey::of_cookie(&opened.cookie).unwrap()).await.unwrap(), Some(user));

		// Telegram: a link, rules, a fan-out, a delivery, a dead chat, the access checks with
		// their pruning, the poller.
		let notifier = |blocked| Notifier {
			panel: panel.clone(),
			bot: FakeBot { blocked },
			concierge: Operators,
			locale: Locale::Ru,
		};
		let token = panel.telegram_link_token(user, Role::Operator, "Olga", now).await.unwrap();
		let start = |chat| Update::Start {
			chat: Chat { id: chat, private: true },
			payload: Some(token.to_string()),
			from: Account::default(),
		};
		notifier(false).handle(start(7), now).await.unwrap();
		panel.telegram_set_rules(user, Role::Operator, &[(Rule::NewLead, true)]).await.unwrap();
		panel.telegram_access_seen(user, Some(Role::Operator), "Olga", now).await.unwrap();
		// By a colleague: the user's own lead above is not told to them.
		let colleague = Actor(uuid::Uuid::now_v7());
		for need in ["a tap", "a sink"] {
			panel.create_lead(colleague, NewLead { need: need.into(), ..new_lead() }, now).await.unwrap();
		}
		assert_eq!(panel.telegram_fan_out(now, Locale::Ru).await.unwrap(), 2, "the colleague's two leads");
		assert_eq!(notifier(false).deliver(now).await.unwrap().sent, 1);
		assert_eq!(notifier(true).deliver(now + SignedDuration::from_secs(2)).await.unwrap().dead, 1);
		assert_eq!(
			notifier(false).recheck_access(now + SignedDuration::from_hours(24 * 8)).await.unwrap(),
			0,
			"a dead chat is not rechecked"
		);
		let holder = uuid::Uuid::now_v7();
		assert_eq!(panel.telegram_poll_lease(holder, now, now + SignedDuration::from_mins(1)).await.unwrap(), Some(0));
		assert!(panel.telegram_poll_advance(holder, 5).await.unwrap());
		panel.telegram_poll_release(holder).await.unwrap();
		notifier(false)
			.handle(
				Update::Stop {
					chat: Chat { id: 7, private: true },
				},
				now,
			)
			.await
			.unwrap();
		assert!(!panel.telegram_settings(user, Role::Operator).await.unwrap().linked);

		// The PostHog import: its lease, its counts, what the screens read of them, a rebuild.
		let site = [BrandId::parse("aquafix").unwrap()].into();
		assert!(panel.add_source("aquafix-site", SourceKind::Site, site).await.unwrap().is_some(), "the brand the import counts");
		let holder = uuid::Uuid::now_v7();
		assert!(panel.posthog_import_lease(holder, now, false).await.unwrap());
		assert_eq!(panel.import_posthog(&OneOfEach, "posthog-1", 3, now).await.unwrap().written, 3);
		panel.posthog_import_release(holder, now, true).await.unwrap();
		assert!(panel.posthog_imported_at().await.unwrap().is_some());
		assert_eq!(panel.site_slices(today, today, None, FunnelBy::All).await.unwrap()[0].sources["direct"], 1);
		assert_eq!(panel.experiments(today, today, None).await.unwrap()[0].variants[0].tally.exposures, 1);
		panel.rebuild_projections().await.unwrap();
		assert_eq!(panel.import_posthog(&OneOfEach, "posthog-1", 3, now).await.unwrap().written, 0, "the rebuilt counts are the same");

		let pool = panel.store().pool();
		for sql in [
			"DELETE FROM events",
			"UPDATE events SET brand_id = 'x'",
			"DELETE FROM sources",
			"UPDATE sources SET brand_ids = '{x}'",
			"CREATE TABLE t (x int)",
			"DROP VIEW reporting.leads",
			"DELETE FROM telegram_poller",
			"UPDATE telegram_link_tokens SET role = 'admin'",
			"DELETE FROM posthog_import",
			"DROP VIEW reporting.experiment_daily",
		] {
			let err = sqlx::query(sqlx::AssertSqlSafe(sql)).execute(pool).await.unwrap_err().to_string();
			assert!(err.contains("permission denied") || err.contains("must be owner"), "{sql}: {err}");
		}
		let n: i64 = sqlx::query_scalar("SELECT count(*) FROM reporting.leads").fetch_one(pool).await.unwrap();
		assert_eq!(n, 4, "the ingested lead and the three taken by phone");
		pool.close().await;
	}
}

#[tokio::test]
async fn grants_go_to_a_plain_role_name_only() {
	let Some(db) = TestDb::create().await else { return };
	Store::migrate(db.options.clone()).await.unwrap();
	for bad in ["", "Panel_App", "panel_app; DROP TABLE events", "1panel", &"a".repeat(64)] {
		let e = Store::grant_runtime(db.options.clone(), bad).await.unwrap_err();
		assert!(format!("{e}").contains("plain lowercase name"), "{bad:?}: {e}");
	}
	let missing = Store::grant_runtime(db.options.clone(), "panel_app_nobody_made").await.unwrap_err();
	assert!(format!("{missing:#}").contains("does the role exist"), "{missing:#}");
}
