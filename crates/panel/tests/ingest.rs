//! Ingest against a real Postgres: signatures, dedup, the registry, brand scoping,
//! projections and their rebuild. Every query of the store runs here at least once.

use jiff::{SignedDuration, Timestamp};
use panel::{
	IngestError, Outcome, Panel,
	testing::{TestDb, event, panel, sign, sign_verbatim},
};
use panel_core::{event::SourceKind, ids::BrandId};
use serde_json::{Value, json};

fn now() -> Timestamp {
	"2026-09-30T18:00:00Z".parse().unwrap()
}

fn at(minutes: i64) -> Timestamp {
	"2026-09-30T09:00:00Z".parse::<Timestamp>().unwrap() + SignedDuration::from_mins(minutes)
}

fn brands(names: &[&str]) -> std::collections::BTreeSet<BrandId> {
	names.iter().map(|b| BrandId::parse(b).unwrap()).collect()
}

/// A panel with a site key for aquafix and a panel key for aquafix; their secrets.
async fn setup(db: &TestDb) -> (Panel, String, String) {
	let panel = panel(db).await;
	let site = panel.add_source("aquafix-site", SourceKind::Site, brands(&["aquafix"])).await.unwrap().unwrap();
	let ops = panel.add_source("aquafix-ops", SourceKind::Panel, brands(&["aquafix"])).await.unwrap().unwrap();
	(panel, site.secret.to_string(), ops.secret.to_string())
}

fn lead(id: &str) -> Value {
	json!({"brandId": "aquafix", "locationId": "paris-11", "leadId": id})
}

fn outcomes(v: &[panel::EventVerdict]) -> Vec<Outcome> {
	v.iter().map(|v| v.outcome.clone()).collect()
}

const ACCEPTED: Outcome = Outcome::Accepted { unregistered: false };

async fn count(pool: &sqlx::PgPool, sql: &'static str) -> i64 {
	sqlx::query_scalar(sql).fetch_one(pool).await.unwrap()
}

#[tokio::test]
async fn signatures() {
	let Some(db) = TestDb::create().await else { return };
	let (panel, secret, ops) = setup(&db).await;
	let events = [event("lead.created", at(0), "site", lead("L-1"), json!({"channel": "form"}))];

	let good = sign("aquafix-site", &secret, &events, now());
	assert_eq!(outcomes(&panel.ingest(good.batch(), now()).await.unwrap()), [ACCEPTED]);

	let wrong_secret = sign("aquafix-site", &"0".repeat(64), &events, now());
	assert!(matches!(panel.ingest(wrong_secret.batch(), now()).await, Err(IngestError::Unauthorized("bad signature"))));

	let stale = sign("aquafix-site", &secret, &events, now() - SignedDuration::from_mins(6));
	assert!(matches!(panel.ingest(stale.batch(), now()).await, Err(IngestError::Unauthorized("stale or malformed timestamp"))));

	let mut tampered = sign("aquafix-site", &secret, &events, now());
	tampered.body.push(b' ');
	assert!(matches!(panel.ingest(tampered.batch(), now()).await, Err(IngestError::Unauthorized("bad signature"))));

	let unknown = sign("nobody", &secret, &events, now());
	assert!(matches!(panel.ingest(unknown.batch(), now()).await, Err(IngestError::Unauthorized("unknown key"))));

	assert!(panel.store().revoke_source("aquafix-site").await.unwrap());
	let revoked = sign("aquafix-site", &secret, &events, now());
	assert!(matches!(panel.ingest(revoked.batch(), now()).await, Err(IngestError::Unauthorized("unknown key"))));
	assert!(!panel.store().revoke_source("aquafix-site").await.unwrap(), "revoked once");

	let not_a_batch = sign("aquafix-ops", &ops, &[], now());
	assert!(matches!(panel.ingest(not_a_batch.batch(), now()).await, Err(IngestError::BadRequest(_))));
}

#[tokio::test]
async fn sources_are_listed_and_unique() {
	let Some(db) = TestDb::create().await else { return };
	let (panel, _, _) = setup(&db).await;
	assert!(
		panel.add_source("aquafix-site", SourceKind::Site, brands(&["vifnet"])).await.unwrap().is_none(),
		"the key id is taken"
	);
	let listed = panel.store().sources().await.unwrap();
	assert_eq!(listed.iter().map(|s| s.grant.key_id.as_str()).collect::<Vec<_>>(), ["aquafix-ops", "aquafix-site"]);
	assert!(listed.iter().all(|s| s.revoked_at.is_none()));
	let pool = db.pool().await;
	let secrets: Vec<Vec<u8>> = sqlx::query_scalar("SELECT secret_sealed FROM sources").fetch_all(&pool).await.unwrap();
	assert!(secrets.iter().all(|s| s.len() == 24 + 64 + 16), "sealed: nonce, 64 hex characters, tag");
}

#[tokio::test]
async fn dedup_by_id() {
	let Some(db) = TestDb::create().await else { return };
	let (panel, secret, _) = setup(&db).await;
	let e = event("lead.created", at(0), "site", lead("L-1"), json!({"channel": "form"}));

	let first = panel.ingest(sign("aquafix-site", &secret, &[e.clone(), e.clone()], now()).batch(), now()).await.unwrap();
	assert_eq!(outcomes(&first), [ACCEPTED, Outcome::Duplicate], "a repeat inside one batch too");
	let again = panel.ingest(sign("aquafix-site", &secret, std::slice::from_ref(&e), now()).batch(), now()).await.unwrap();
	assert_eq!(outcomes(&again), [Outcome::Duplicate]);

	let mut changed = e.clone();
	changed["properties"]["channel"] = json!("phone_inbound");
	let conflict = panel.ingest(sign("aquafix-site", &secret, &[changed], now()).batch(), now()).await.unwrap();
	assert_eq!(outcomes(&conflict), [Outcome::Rejected(panel_core::Invalid::new("id already names a different event"))]);

	let pool = db.pool().await;
	assert_eq!(count(&pool, "SELECT count(*) FROM events").await, 1);
	assert_eq!(
		count(&pool, "SELECT count(*) FROM leads WHERE channel = 'form'").await,
		1,
		"the conflicting resend changed nothing"
	);
}

#[tokio::test]
async fn unregistered_types_are_kept_not_projected() {
	let Some(db) = TestDb::create().await else { return };
	let (panel, secret, _) = setup(&db).await;
	let mut future = event("lead.created", at(0), "site", lead("L-9"), json!({"channel": "form", "utm": "gbp"}));
	future["typeVersion"] = json!(2);
	let events = [event("review.new", at(0), "site", json!({"brandId": "aquafix"}), json!({"stars": 2})), future];

	let got = panel.ingest(sign("aquafix-site", &secret, &events, now()).batch(), now()).await.unwrap();
	assert_eq!(outcomes(&got), [Outcome::Accepted { unregistered: true }, Outcome::Accepted { unregistered: true }]);
	let pool = db.pool().await;
	assert_eq!(count(&pool, "SELECT count(*) FROM events WHERE status = 'unregistered'").await, 2);
	assert_eq!(count(&pool, "SELECT count(*) FROM leads").await, 0);
	assert_eq!(count(&pool, "SELECT sum(events)::int8 FROM reporting.ingest_daily WHERE status = 'unregistered'").await, 2);
}

#[tokio::test]
async fn a_key_writes_only_its_brands_and_kind() {
	let Some(db) = TestDb::create().await else { return };
	let (panel, secret, _) = setup(&db).await;
	let events = [
		event("lead.created", at(0), "site", json!({"brandId": "vifnet", "leadId": "V-1"}), json!({"channel": "form"})),
		event("lead.contacted", at(1), "panel", lead("L-1"), json!({})),
		event("lead.created", at(0), "site", lead("L-1"), json!({"channel": "form"})),
	];
	let got = panel.ingest(sign("aquafix-site", &secret, &events, now()).batch(), now()).await.unwrap();
	let [foreign, posing, ok] = &got[..] else { panic!("{got:?}") };
	assert_eq!(foreign.outcome, Outcome::Rejected(panel_core::Invalid::new("this key may not write for brand vifnet")));
	assert!(matches!(&posing.outcome, Outcome::Rejected(e) if e.0.contains("source.kind panel")), "{posing:?}");
	assert_eq!(ok.outcome, ACCEPTED);
	assert_eq!((foreign.index, ok.index), (0, 2));
	assert_eq!(foreign.id, events[0]["id"].as_str().unwrap());
	let pool = db.pool().await;
	assert_eq!(count(&pool, "SELECT count(*) FROM events").await, 1, "rejected events are not journaled");
}

#[tokio::test]
async fn invalid_events_of_known_types_are_rejected() {
	let Some(db) = TestDb::create().await else { return };
	let (panel, _, ops) = setup(&db).await;
	let events = [
		event("call.logged", at(0), "panel", lead("L-1"), json!({"outcome": "voicemail"})),
		event("job.won", at(0), "panel", lead("L-1"), json!({})),
		event("payment.received", at(0), "panel", lead("L-1"), json!({"billed": 100, "commission": 200, "currency": "EUR"})),
		json!({"id": "not even close"}),
	];
	let got = panel.ingest(sign("aquafix-ops", &ops, &events, now()).batch(), now()).await.unwrap();
	assert!(got.iter().all(|v| matches!(v.outcome, Outcome::Rejected(_))), "{got:?}");
	assert_eq!(got[3].id, "not even close");
	assert_eq!(count(&db.pool().await, "SELECT count(*) FROM events").await, 0);
}

/// One lead through the whole funnel, delivered out of order over two sources and three
/// batches, plus a lost lead.
async fn scenario(panel: &Panel, site: &str, ops: &str) {
	let created = event("lead.created", at(0), "site", lead("L-1"), json!({"channel": "form"}));
	let mut with_pii = event("lead.created", at(5), "panel", lead("L-2"), json!({"channel": "phone_inbound", "enteredBy": "op-1"}));
	with_pii["pii"] = json!({"phone": "+33 6 12 34 56 78", "need": "leak under the sink"});
	let job = json!({"brandId": "aquafix", "locationId": "paris-11", "leadId": "L-1", "jobId": "J-1"});
	let late = [
		event(
			"payment.received",
			at(300),
			"panel",
			job.clone(),
			json!({"billed": "12000", "commission": 1800, "currency": "EUR"}),
		),
		event("job.completed", at(240), "panel", job.clone(), json!({})),
		event("job.won", at(60), "panel", job, json!({})),
		event("lead.quoted", at(30), "panel", lead("L-1"), json!({"amount": 12000, "currency": "EUR"})),
	];
	let early = [
		event("call.attempted", at(2), "panel", lead("L-1"), json!({})),
		event("call.logged", at(3), "panel", lead("L-1"), json!({"outcome": "answered"})),
		event("lead.contacted", at(3), "panel", lead("L-1"), json!({"channel": "phone"})),
		with_pii,
		event("call.logged", at(6), "panel", lead("L-2"), json!({"outcome": "no_answer"})),
		event("lead.lost", at(7), "panel", lead("L-2"), json!({"reason": "no_answer"})),
	];
	for (key, secret, batch) in [("aquafix-ops", ops, &late[..]), ("aquafix-site", site, &[created][..]), ("aquafix-ops", ops, &early[..])] {
		let got = panel.ingest(sign(key, secret, batch, now()).batch(), now()).await.unwrap();
		assert!(got.iter().all(|v| v.outcome == ACCEPTED), "{got:?}");
	}
}

/// The projections as rows of JSON, in a stable order: what "the same state" means.
async fn projections(pool: &sqlx::PgPool) -> Vec<Value> {
	sqlx::query_scalar(
		"SELECT to_jsonb(t) FROM (SELECT 'lead' AS kind, to_jsonb(l) AS row FROM leads l \
		 UNION ALL SELECT 'call', to_jsonb(c) FROM calls c \
		 UNION ALL SELECT 'payment', to_jsonb(p) FROM payments p) t ORDER BY kind, row::text",
	)
	.fetch_all(pool)
	.await
	.unwrap()
}

#[tokio::test]
async fn stages_are_projected_from_the_events() {
	let Some(db) = TestDb::create().await else { return };
	let (panel, site, ops) = setup(&db).await;
	scenario(&panel, &site, &ops).await;
	let pool = db.pool().await;

	type LeadRow = (String, String, bool, Option<String>, Option<String>);
	let rows: Vec<LeadRow> = sqlx::query_as("SELECT lead_id, stage, manual, lost_reason, job_id FROM leads ORDER BY lead_id")
		.fetch_all(&pool)
		.await
		.unwrap();
	assert_eq!(
		rows,
		[
			("L-1".into(), "paid".into(), false, None, Some("J-1".into())),
			("L-2".into(), "lost".into(), true, Some("no_answer".into()), None),
		]
	);
	let times: Vec<Option<chrono::DateTime<chrono::Utc>>> = sqlx::query_as::<
		_,
		(
			Option<chrono::DateTime<chrono::Utc>>,
			Option<chrono::DateTime<chrono::Utc>>,
			Option<chrono::DateTime<chrono::Utc>>,
			Option<chrono::DateTime<chrono::Utc>>,
		),
	>("SELECT created_at, contacted_at, won_at, paid_at FROM leads WHERE lead_id = 'L-1'")
	.fetch_one(&pool)
	.await
	.map(|(a, b, c, d)| vec![a, b, c, d])
	.unwrap();
	let pg = |m| Some(chrono::DateTime::from_timestamp(at(m).as_second(), 0).unwrap());
	assert_eq!(times, [pg(0), pg(3), pg(60), pg(300)]);

	assert_eq!(count(&pool, "SELECT count(*) FROM calls").await, 3);
	assert_eq!(count(&pool, "SELECT count(*) FROM calls WHERE kind = 'logged' AND outcome IS NOT NULL").await, 2);
	let (billed, commission): (i64, i64) = sqlx::query_as("SELECT billed, commission FROM payments").fetch_one(&pool).await.unwrap();
	assert_eq!((billed, commission), (12_000, 1_800));

	let funnel: (i64, i64, i64, i64, i64) = sqlx::query_as("SELECT leads, contacted, paid, lost_now, manual FROM reporting.funnel_daily")
		.fetch_one(&pool)
		.await
		.unwrap();
	assert_eq!(funnel, (2, 1, 1, 1, 1));
}

#[tokio::test]
async fn rebuild_lands_on_the_same_state() {
	let Some(db) = TestDb::create().await else { return };
	let (panel, site, ops) = setup(&db).await;
	scenario(&panel, &site, &ops).await;
	let pool = db.pool().await;
	let incremental = projections(&pool).await;
	assert_eq!(incremental.len(), 2 + 3 + 1);

	let rebuilt = panel.rebuild_projections().await.unwrap();
	assert_eq!((rebuilt.events, rebuilt.registered, rebuilt.leads), (11, 11, 2));
	assert_eq!(projections(&pool).await, incremental);
}

#[tokio::test]
async fn rebuild_picks_up_a_type_registered_after_it_arrived() {
	let Some(db) = TestDb::create().await else { return };
	let (panel, site, _) = setup(&db).await;
	let pool = db.pool().await;
	panel
		.ingest(
			sign("aquafix-site", &site, &[event("lead.created", at(0), "site", lead("L-1"), json!({"channel": "form"}))], now()).batch(),
			now(),
		)
		.await
		.unwrap();
	// As an older panel that did not know lead.created would have left it.
	sqlx::query("UPDATE events SET status = 'unregistered'").execute(&pool).await.unwrap();
	sqlx::query("TRUNCATE leads").execute(&pool).await.unwrap();

	let rebuilt = panel.rebuild_projections().await.unwrap();
	assert_eq!((rebuilt.registered, rebuilt.unregistered, rebuilt.leads), (1, 0, 1));
	assert_eq!(count(&pool, "SELECT count(*) FROM events WHERE status = 'registered'").await, 1);
	assert_eq!(count(&pool, "SELECT count(*) FROM leads").await, 1);
}

#[tokio::test]
async fn the_journal_is_append_only() {
	let Some(db) = TestDb::create().await else { return };
	let (panel, site, _) = setup(&db).await;
	panel
		.ingest(
			sign("aquafix-site", &site, &[event("lead.created", at(0), "site", lead("L-1"), json!({"channel": "form"}))], now()).batch(),
			now(),
		)
		.await
		.unwrap();
	let pool = db.pool().await;
	for sql in [
		"UPDATE events SET brand_id = 'vifnet'",
		"UPDATE events SET properties = '{}'",
		"DELETE FROM events",
		"TRUNCATE events CASCADE",
	] {
		let err = sqlx::query(sql).execute(&pool).await.unwrap_err().to_string();
		assert!(err.contains("append-only"), "{sql}: {err}");
	}
	sqlx::query("UPDATE events SET status = 'invalid', status_reason = 'x'")
		.execute(&pool)
		.await
		.expect("status is the one thing that changes");
}

#[tokio::test]
async fn pii_is_sealed_and_kept_out_of_reporting() {
	let Some(db) = TestDb::create().await else { return };
	let (panel, site, ops) = setup(&db).await;
	scenario(&panel, &site, &ops).await;
	let pool = db.pool().await;

	let (id, sealed): (uuid::Uuid, Vec<u8>) = sqlx::query_as("SELECT id, pii_sealed FROM events WHERE pii_sealed IS NOT NULL").fetch_one(&pool).await.unwrap();
	assert!(!sealed.windows(4).any(|w| w == b"+33 "), "no plaintext phone at rest");
	let pii = panel.pii(id).await.unwrap().unwrap();
	assert_eq!(pii["phone"], "+33 6 12 34 56 78");
	assert_eq!(panel.pii(uuid::Uuid::now_v7()).await.unwrap(), None);

	let columns: Vec<String> = sqlx::query_scalar("SELECT table_name || '.' || column_name FROM information_schema.columns WHERE table_schema = 'reporting'")
		.fetch_all(&pool)
		.await
		.unwrap();
	assert!(columns.len() > 20, "{columns:?}");
	for c in &columns {
		assert!(!["pii", "properties", "secret", "sealed"].iter().any(|bad| c.contains(bad)), "{c} in reporting");
	}
}

#[tokio::test]
async fn a_site_key_writes_no_operator_events() {
	let Some(db) = TestDb::create().await else { return };
	let (panel, site, _) = setup(&db).await;
	let events = [
		event("payment.received", at(0), "site", lead("L-1"), json!({"billed": 100, "commission": 10, "currency": "EUR"})),
		event("job.won", at(0), "site", json!({"brandId": "aquafix", "leadId": "L-1", "jobId": "J-1"}), json!({})),
		event("call.logged", at(0), "site", lead("L-1"), json!({"outcome": "answered"})),
	];
	let got = panel.ingest(sign("aquafix-site", &site, &events, now()).batch(), now()).await.unwrap();
	assert_eq!(got[0].outcome, Outcome::Rejected(panel_core::Invalid::new("a site source may not write payment.received")));
	assert!(got.iter().all(|v| matches!(v.outcome, Outcome::Rejected(_))), "{got:?}");
	let pool = db.pool().await;
	assert_eq!(count(&pool, "SELECT count(*) FROM events").await, 0);

	// One that got into the journal anyway — an older panel, a hand-made row — is dropped
	// by the rebuild, which judges through the same rule.
	panel
		.ingest(
			sign("aquafix-site", &site, &[event("lead.created", at(0), "site", lead("L-1"), json!({"channel": "form"}))], now()).batch(),
			now(),
		)
		.await
		.unwrap();
	sqlx::query(
		"INSERT INTO events (id, schema, type, type_version, occurred_at, received_at, source_kind, source_id, brand_id, lead_id, properties, content_mac, status) \
		 VALUES ($1, 'sa.funnel.v1', 'payment.received', 1, now(), now(), 'site', 'x', 'aquafix', 'L-1', '{\"billed\": 100, \"commission\": 10, \"currency\": \"EUR\"}', $2, 'registered')",
	)
	.bind(uuid::Uuid::now_v7())
	.bind(vec![0u8; 32])
	.execute(&pool)
	.await
	.unwrap();
	let rebuilt = panel.rebuild_projections().await.unwrap();
	assert_eq!((rebuilt.registered, rebuilt.invalid), (1, 1));
	assert_eq!(count(&pool, "SELECT count(*) FROM payments").await, 0);
	assert_eq!(
		count(&pool, "SELECT count(*) FROM events WHERE status = 'invalid' AND status_reason LIKE 'a site source%'").await,
		1
	);
}

#[tokio::test]
async fn a_back_dated_creation_does_not_take_over_a_lead() {
	let Some(db) = TestDb::create().await else { return };
	let (panel, site, ops) = setup(&db).await;
	let by_hand = event("lead.created", at(10), "panel", lead("L-1"), json!({"channel": "phone_inbound", "enteredBy": "op-1"}));
	panel.ingest(sign("aquafix-ops", &ops, &[by_hand], now()).batch(), now()).await.unwrap();
	let back_dated = event("lead.created", at(0), "site", lead("L-1"), json!({"channel": "form"}));
	let later = now() + SignedDuration::from_mins(1);
	let got = panel.ingest(sign("aquafix-site", &site, &[back_dated], later).batch(), later).await.unwrap();
	assert_eq!(outcomes(&got), [ACCEPTED], "journaled: it is a fact that the site said so");

	let pool = db.pool().await;
	let lead_row = || async {
		sqlx::query_as::<_, (bool, Option<String>, chrono::DateTime<chrono::Utc>)>("SELECT manual, channel, created_at FROM leads WHERE lead_id = 'L-1'")
			.fetch_one(&pool)
			.await
			.unwrap()
	};
	let (manual, channel, created) = lead_row().await;
	assert!(manual);
	assert_eq!(channel.as_deref(), Some("phone_inbound"));
	assert_eq!(created.timestamp(), at(10).as_second());
	panel.rebuild_projections().await.unwrap();
	assert_eq!(lead_row().await, (manual, channel, created), "and the rebuild agrees");
}

#[tokio::test]
async fn what_postgres_would_refuse_is_rejected_not_a_500() {
	let Some(db) = TestDb::create().await else { return };
	let (panel, site, _) = setup(&db).await;
	let mut huge_version = event("lead.created", at(0), "site", lead("L-1"), json!({"channel": "form"}));
	huge_version["typeVersion"] = json!(u32::MAX);
	let events = [
		huge_version,
		event("review.new", at(0), "site", json!({"brandId": "aquafix"}), json!({"text": "nul \u{0} here"})),
		event("review.new", at(0), "site", json!({"brandId": "aquafix"}), json!({"text": "x".repeat(17 * 1024)})),
		event("lead.created", at(0), "site", lead("L-2"), json!({"channel": "form"})),
	];
	let got = panel
		.ingest(sign("aquafix-site", &site, &events, now()).batch(), now())
		.await
		.expect("per event, not for the batch");
	assert!(got[..3].iter().all(|v| matches!(v.outcome, Outcome::Rejected(_))), "{got:?}");
	assert_eq!(got[3].outcome, ACCEPTED, "the rest of the batch goes on");
	assert_eq!(count(&db.pool().await, "SELECT count(*) FROM events").await, 1);
}

#[tokio::test]
async fn a_source_is_named_by_its_key() {
	let Some(db) = TestDb::create().await else { return };
	let (panel, site, _) = setup(&db).await;
	let mut e = event("lead.created", at(0), "site", lead("L-1"), json!({"channel": "form"}));
	e["source"]["id"] = json!("vifnet-site");
	let got = panel.ingest(sign_verbatim("aquafix-site", &site, &[e], now()).batch(), now()).await.unwrap();
	assert_eq!(outcomes(&got), [Outcome::Rejected(panel_core::Invalid::new("source.id is not the id of the key that signed it"))]);
	let e = event("lead.created", at(0), "site", lead("L-1"), json!({"channel": "form"}));
	let got = panel.ingest(sign("aquafix-site", &site, &[e], now()).batch(), now()).await.unwrap();
	assert_eq!(outcomes(&got), [ACCEPTED]);
	let source_id: String = sqlx::query_scalar("SELECT source_id FROM events").fetch_one(&db.pool().await).await.unwrap();
	assert_eq!(source_id, "aquafix-site");
}
