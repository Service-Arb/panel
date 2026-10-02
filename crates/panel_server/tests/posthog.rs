//! The PostHog import end to end on a real Postgres, against a mock query API served
//! in-process (axum on a local port — never the network): the HogQL answer read, the counts
//! journaled and projected, a recount replacing what changed and nothing else, the lease.

use std::sync::{Arc, Mutex};

use axum::{
	Json, Router,
	extract::{Path, State},
	http::{HeaderMap, StatusCode},
	routing::post,
};
use jiff::Timestamp;
use panel::{
	Panel,
	operator::FunnelBy,
	posthog::WINDOW_DAYS,
	testing::{TestDb, panel},
};
use panel_core::{
	event::SourceKind,
	ids::BrandId,
	metrics::{IntentChannel, Tally},
};
use panel_server::posthog::{QueryApi, once};
use serde_json::{Value, json};
use uuid::Uuid;

const KEY: &str = "phx_test_key";
const PROJECT: &str = "4242";

/// What the mock answers, per query: the rows of each table, or a status to fail with.
#[derive(Default)]
struct Mock {
	visits: Vec<Value>,
	intents: Vec<Value>,
	experiments: Vec<Value>,
	fail: Option<StatusCode>,
	/// Every request body, in order.
	asked: Vec<Value>,
}

#[derive(Clone, Default)]
struct MockApi(Arc<Mutex<Mock>>);

impl MockApi {
	fn with<R>(&self, f: impl FnOnce(&mut Mock) -> R) -> R {
		f(&mut self.0.lock().unwrap())
	}
}

async fn query(State(mock): State<MockApi>, Path(project): Path<String>, headers: HeaderMap, Json(body): Json<Value>) -> (StatusCode, Json<Value>) {
	if headers.get("authorization").and_then(|v| v.to_str().ok()) != Some(&format!("Bearer {KEY}")) || project != PROJECT {
		return (StatusCode::UNAUTHORIZED, Json(json!({"detail": "bad key"})));
	}
	mock.with(|m| {
		m.asked.push(body.clone());
		if let Some(status) = m.fail {
			return (status, Json(json!({"detail": "query failed"})));
		}
		let hogql = body["query"]["query"].as_str().unwrap_or_default();
		let rows = if hogql.contains("'location_page_view'") {
			&m.visits
		} else if hogql.contains("'contact_intent_click'") {
			&m.intents
		} else {
			&m.experiments
		};
		(StatusCode::OK, Json(json!({"columns": ["…"], "results": rows, "hasMore": false})))
	})
}

async fn serve_mock(mock: MockApi) -> String {
	let app = Router::new().route("/api/projects/{project}/query/", post(query)).with_state(mock);
	let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
	let addr = listener.local_addr().unwrap();
	// Dropped with the test's runtime.
	let _server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
	format!("http://{addr}")
}

fn today() -> String {
	Timestamp::now().to_zoned(jiff::tz::TimeZone::UTC).date().to_string()
}

async fn setup(db: &TestDb) -> (Panel, MockApi, QueryApi, String) {
	let panel = panel(db).await;
	// The import counts the brands a source key writes for; vifnet has none.
	panel
		.add_source("aquafix-site", SourceKind::Site, [BrandId::parse("aquafix").unwrap()].into())
		.await
		.unwrap()
		.unwrap();
	let mock = MockApi::default();
	let base = serve_mock(mock.clone()).await;
	let api = QueryApi::new(&base, PROJECT, KEY).unwrap();
	(panel, mock, api, base)
}

async fn import(panel: &Panel, api: &QueryApi) -> panel::posthog::Imported {
	once(panel, api, &api.source_id(), Uuid::now_v7(), WINDOW_DAYS, true).await.unwrap().unwrap()
}

async fn events_of(db: &TestDb, r#type: &str) -> Vec<(Value, String)> {
	sqlx::query_as::<_, (sqlx::types::Json<Value>, String)>(
		"SELECT properties, source_id FROM events WHERE type = $1 AND source_kind = 'posthog' AND key_id IS NULL ORDER BY received_at, id",
	)
	.bind(r#type)
	.fetch_all(&db.pool().await)
	.await
	.unwrap()
	.into_iter()
	.map(|(p, s)| (p.0, s))
	.collect()
}

#[tokio::test]
async fn a_recount_replaces_what_changed_and_nothing_else() {
	let db = TestDb::create().await;
	let (panel, mock, api, _) = setup(&db).await;
	let day = today();
	mock.with(|m| {
		m.visits = vec![
			json!([day, "aquafix", "paris-11", "google.com", 40]),
			json!([day, "aquafix", "paris-11", "gbp", 12]),
			json!([day, "aquafix", null, "direct", 3]),
			json!([day, "vifnet", "lyon-2", "direct", 99]),
		];
		m.intents = vec![json!([day, "aquafix", "paris-11", "phone", 5]), json!([day, "aquafix", "paris-11", "form_open", 2])];
		m.experiments = vec![
			json!([day, "aquafix", "hero", "a", "experiment_exposed", null, 150]),
			json!([day, "aquafix", "hero", "b", "experiment_exposed", null, 140]),
			json!([day, "aquafix", "hero", "b", "experiment_contact", "phone", 6]),
			json!([day, "aquafix", "hero", "b", "experiment_lead", null, "3"]),
		];
	});

	let first = import(&panel, &api).await;
	assert_eq!((first.written, first.skipped, first.conflicts), (7, 1, 0), "vifnet has no source key: left out");
	let asked = mock.with(|m| std::mem::take(&mut m.asked));
	assert_eq!(asked.len(), 3);
	assert_eq!(asked[0]["query"]["kind"], "HogQLQuery");
	assert!(asked[0]["query"]["values"]["from"].as_str().unwrap().ends_with("T00:00:00Z"), "{}", asked[0]);
	assert!(asked[2]["query"]["query"].as_str().unwrap().contains("forced"), "QA visits are left out of the experiments");

	let visits = events_of(&db, "site.metrics").await;
	assert_eq!(visits.len(), 3);
	assert!(visits.iter().all(|(p, s)| p["revision"] == 1 && s == "posthog-4242"), "{visits:?}");

	let today: jiff::civil::Date = day.parse().unwrap();
	let slices = panel.site_slices(today, today, None, FunnelBy::Location).await.unwrap();
	assert_eq!(slices.len(), 2, "paris-11, and the pages naming no location");
	assert_eq!(slices[0].location.as_deref(), Some("paris-11"));
	assert_eq!(slices[0].days[&today].visits, 52);
	assert_eq!(slices[0].days[&today].intents[&IntentChannel::Phone], 5);
	assert_eq!(slices[1].location, None);
	let exp = panel.experiments(today, today, None).await.unwrap();
	assert_eq!((exp.len(), exp[0].control.as_str()), (1, "a"));
	assert_eq!(
		exp[0].variants[1].tally,
		Tally {
			exposures: 140,
			leads: 3,
			phone: 6,
			..Tally::default()
		}
	);

	// The same counts again: nothing to write.
	let again = import(&panel, &api).await;
	assert_eq!(again.written, 0);
	assert_eq!(events_of(&db, "site.metrics").await.len(), 3, "the journal does not grow on a recount that changed nothing");

	// Later the same day: google grew, gbp vanished (back to 0), the rest unchanged.
	mock.with(|m| {
		m.visits = vec![json!([day, "aquafix", "paris-11", "google.com", 45]), json!([day, "aquafix", null, "direct", 3])];
	});
	let third = import(&panel, &api).await;
	assert_eq!(third.written, 2, "google's new count, gbp's zero");
	let visits = events_of(&db, "site.metrics").await;
	let revisions: Vec<(String, i64, i64)> = visits
		.iter()
		.map(|(p, _)| (p["source"].as_str().unwrap().to_owned(), p["visits"].as_i64().unwrap(), p["revision"].as_i64().unwrap()))
		.collect();
	assert!(revisions.contains(&("google.com".into(), 45, 2)) && revisions.contains(&("gbp".into(), 0, 2)), "{revisions:?}");
	let slices = panel.site_slices(today, today, None, FunnelBy::All).await.unwrap();
	assert_eq!(slices[0].days[&today].visits, 48, "the newest count of each slice, not their sum");
	assert_eq!(slices[0].sources.get("gbp"), Some(&0));

	// The projection is a function of the journal.
	panel.rebuild_projections().await.unwrap();
	assert_eq!(panel.site_slices(today, today, None, FunnelBy::All).await.unwrap(), slices);
	assert!(panel.posthog_imported_at().await.unwrap().is_some());
}

#[tokio::test]
async fn a_failing_query_journals_nothing_and_frees_the_lease() {
	let db = TestDb::create().await;
	let (panel, mock, api, base) = setup(&db).await;
	mock.with(|m| m.fail = Some(StatusCode::TOO_MANY_REQUESTS));
	let err = once(&panel, &api, &api.source_id(), Uuid::now_v7(), WINDOW_DAYS, true).await.unwrap_err();
	assert!(format!("{err:#}").contains("429"), "{err:#}");
	assert!(!format!("{err:#}").contains(KEY), "the key never shows");
	assert_eq!(panel.posthog_imported_at().await.unwrap(), None);

	// A full table is not read as complete.
	mock.with(|m| {
		m.fail = None;
		m.visits = vec![json!([today(), "aquafix", "paris-11", "direct", 1]); panel::posthog::ROW_LIMIT];
	});
	let err = once(&panel, &api, &api.source_id(), Uuid::now_v7(), WINDOW_DAYS, true).await.unwrap_err();
	assert!(format!("{err:#}").contains("fewer days"), "{err:#}");

	let wrong_key = QueryApi::new(&base, PROJECT, "phx_other").unwrap();
	let err = once(&panel, &wrong_key, &api.source_id(), Uuid::now_v7(), WINDOW_DAYS, true).await.unwrap_err();
	assert!(format!("{err:#}").contains("401"), "{err:#}");
	assert!(events_of(&db, "site.metrics").await.is_empty());
}

#[tokio::test]
async fn one_replica_imports_once_an_hour() {
	let db = TestDb::create().await;
	let (panel, ..) = setup(&db).await;
	let now = Timestamp::now();
	let (a, b) = (Uuid::now_v7(), Uuid::now_v7());
	assert!(panel.posthog_import_lease(a, now, false).await.unwrap(), "due: never imported");
	assert!(!panel.posthog_import_lease(b, now, false).await.unwrap(), "held by a");
	assert!(!panel.posthog_import_lease(b, now, true).await.unwrap(), "held, even for the CLI");
	panel.posthog_import_release(a, now, true).await.unwrap();
	let later = |mins| now + jiff::SignedDuration::from_mins(mins);
	assert!(!panel.posthog_import_lease(b, later(30), false).await.unwrap(), "imported half an hour ago");
	assert!(panel.posthog_import_lease(b, later(61), false).await.unwrap(), "an hour on");
	// b dies holding it: the lease lapses.
	assert!(!panel.posthog_import_lease(a, later(65), false).await.unwrap());
	assert!(panel.posthog_import_lease(a, later(61 + 11), false).await.unwrap(), "taken over once lapsed");
	panel.posthog_import_release(a, later(72), false).await.unwrap();
	assert!(!panel.posthog_import_lease(b, later(75), false).await.unwrap(), "a failed try waits ten minutes");
	assert!(panel.posthog_import_lease(b, later(83), false).await.unwrap());
}
