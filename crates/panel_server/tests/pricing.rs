//! A brand's pricing over HTTP, end to end on the real router: the editor's routes behind the
//! sign-in (dev sign-in for the session, so the gate and the role run unchanged), the preview
//! held to kitstart's cases, and the sites' read under `/api/internal`.

use std::collections::HashMap;

use axum::{
	Router,
	body::{Body, to_bytes},
	http::{Method, Request, StatusCode, header},
};
use panel::testing::{TestDb, panel};
use panel_core::role::Role;
use panel_server::{
	concierge::{Concierge, DevIdentity},
	http,
	signin::{SignIn, SignInConfig},
};
use serde_json::{Value, json};
use tower::ServiceExt;

const ORIGIN: &str = "http://127.0.0.1:59120";
const ITEM: &str = "/api/v1/pricing/vifnet";
const LIVE: &str = "/api/internal/brands/vifnet/pricing?locale=fr";

fn app(panel: panel::Panel, role: Role) -> Router {
	let who = DevIdentity {
		role,
		email: format!("dev-{}@localhost", role.as_str()),
	};
	let config = SignInConfig {
		panel_origin: ORIGIN.to_owned(),
		concierge_origin: ORIGIN.to_owned(),
	};
	http::app(SignIn::new(panel, Concierge::dev(who), config))
}

fn fixture(path: &str) -> Value {
	let path = format!("{}/../panel_core/tests/fixtures/pricing/{path}", env!("CARGO_MANIFEST_DIR"));
	serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap()
}

#[derive(Default)]
struct Browser {
	jar: HashMap<String, String>,
}

impl Browser {
	async fn send(&mut self, app: &Router, method: Method, uri: &str, body: Option<Value>, csrf: bool) -> (StatusCode, Value) {
		let mut req = Request::builder().method(method).uri(uri);
		if !self.jar.is_empty() {
			req = req.header(header::COOKIE, self.jar.iter().map(|(k, v)| format!("{k}={v}")).collect::<Vec<_>>().join("; "));
		}
		if csrf && let Some(t) = self.jar.get("sa_csrf") {
			req = req.header("x-sa-csrf", t);
		}
		let req = match body {
			Some(b) => req.header(header::CONTENT_TYPE, "application/json").body(Body::from(b.to_string())),
			None => req.body(Body::empty()),
		};
		let res = app.clone().oneshot(req.unwrap()).await.unwrap();
		for c in res.headers().get_all(header::SET_COOKIE) {
			let c = c.to_str().unwrap();
			let (pair, attrs) = c.split_once(';').unwrap_or((c, ""));
			let (k, v) = pair.split_once('=').unwrap();
			if attrs.contains("Max-Age=0") {
				self.jar.remove(k);
			} else {
				self.jar.insert(k.to_owned(), v.to_owned());
			}
		}
		let status = res.status();
		let bytes = to_bytes(res.into_body(), usize::MAX).await.unwrap();
		(status, serde_json::from_slice(&bytes).unwrap_or(Value::Null))
	}

	async fn get(&mut self, app: &Router, uri: &str) -> (StatusCode, Value) {
		self.send(app, Method::GET, uri, None, false).await
	}

	async fn write(&mut self, app: &Router, method: Method, uri: &str, body: Value) -> (StatusCode, Value) {
		self.send(app, method, uri, Some(body), true).await
	}

	async fn signed_in(app: &Router) -> Self {
		let mut b = Self::default();
		let login = app.clone().oneshot(Request::get("/auth/login").body(Body::empty()).unwrap()).await.unwrap();
		let location = login.headers()[header::LOCATION].to_str().unwrap().to_owned();
		for c in login.headers().get_all(header::SET_COOKIE) {
			let (pair, _) = c.to_str().unwrap().split_once(';').unwrap();
			let (k, v) = pair.split_once('=').unwrap();
			b.jar.insert(k.to_owned(), v.to_owned());
		}
		let (status, _) = b.get(app, location.strip_prefix(ORIGIN).unwrap()).await;
		assert_eq!(status, StatusCode::SEE_OTHER);
		assert!(b.jar.contains_key("sa_session"));
		b
	}
}

#[tokio::test]
async fn an_operator_reads_and_previews_but_changes_nothing() {
	let db = TestDb::create().await;
	let app = app(panel(&db).await, Role::Operator);
	let mut b = Browser::signed_in(&app).await;

	let (status, item) = b.get(&app, ITEM).await;
	assert_eq!(status, StatusCode::OK, "{item}");
	assert_eq!(
		item,
		json!({"brand_id": "vifnet", "locales": ["fr", "en"], "model": null, "updated_at": null, "updated_by": null})
	);
	assert_eq!(b.get(&app, "/api/v1/pricing").await, (StatusCode::OK, json!({"items": []})), "no brand known yet");
	assert_eq!(b.get(&app, &format!("{ITEM}/changes")).await, (StatusCode::OK, json!({"changes": []})));

	let preview = json!({"model": fixture("valid/cleaning.json"), "need": "windows", "inputs": {}});
	assert_eq!(b.write(&app, Method::POST, &format!("{ITEM}/preview"), preview).await, (StatusCode::OK, json!({"cents": 8900})));

	let put = json!({"model": fixture("valid/cleaning.json"), "expected_updated_at": null});
	assert_eq!(b.write(&app, Method::PUT, ITEM, put).await.0, StatusCode::FORBIDDEN);
	assert_eq!(b.write(&app, Method::DELETE, ITEM, json!({"expected_updated_at": null})).await.0, StatusCode::FORBIDDEN);
	assert_eq!(b.get(&app, "/api/v1/pricing/Vifnet").await.0, StatusCode::BAD_REQUEST, "not a brand id");
}

#[tokio::test]
async fn an_admin_saves_a_model_and_the_sites_read_it() {
	let db = TestDb::create().await;
	let panel = panel(&db).await;
	let app = app(panel.clone(), Role::Admin);
	let mut b = Browser::signed_in(&app).await;
	let mut site = Browser::default();
	let cleaning = fixture("valid/cleaning.json");

	assert_eq!(site.get(&app, LIVE).await, (StatusCode::OK, json!({})), "never set: {{}}, never a 404");
	for odd in ["/api/internal/brands/Vifnet/pricing", "/api/internal/brands/nobody-knows/pricing"] {
		assert_eq!(site.get(&app, odd).await, (StatusCode::OK, json!({})), "{odd}");
	}

	let put = json!({"model": cleaning, "expected_updated_at": null});
	let (status, _) = b.send(&app, Method::PUT, ITEM, Some(put.clone()), false).await;
	assert_eq!(status, StatusCode::FORBIDDEN, "a write without the CSRF header");

	let mut invalid = cleaning.clone();
	invalid["needs"]["standard"]["inputs"][2] = json!("pets");
	let (status, body) = b.write(&app, Method::PUT, ITEM, json!({"model": invalid, "expected_updated_at": null})).await;
	assert_eq!(
		(status, body),
		(StatusCode::UNPROCESSABLE_ENTITY, json!({"error": "no input \"pets\"", "path": "needs.standard.inputs[2]"}))
	);
	let mut unlabelled = cleaning.clone();
	unlabelled["inputs"][1]["options"][0]["labels"] = json!({"fr": "Studio"});
	let (status, body) = b.write(&app, Method::PUT, ITEM, json!({"model": unlabelled, "expected_updated_at": null})).await;
	assert_eq!(
		(status, body),
		(StatusCode::UNPROCESSABLE_ENTITY, json!({"error": "no \"en\" label", "path": "inputs[1].options[0].labels"}))
	);
	let (status, body) = b.write(&app, Method::PUT, ITEM, json!({"model": [], "expected_updated_at": null})).await;
	assert_eq!((status, body), (StatusCode::UNPROCESSABLE_ENTITY, json!({"error": "an object", "path": ""})));

	let (status, saved) = b.write(&app, Method::PUT, ITEM, put.clone()).await;
	assert_eq!(status, StatusCode::OK, "{saved}");
	assert_eq!(saved["model"], cleaning);
	assert_eq!(saved["updated_by"], "dev-admin@localhost");
	assert_eq!(saved["locales"], json!(["fr", "en"]));
	let updated_at = saved["updated_at"].clone();
	assert!(updated_at.is_string());
	assert_eq!(site.get(&app, LIVE).await, (StatusCode::OK, cleaning.clone()), "the whole model, every locale's labels");
	assert_eq!(b.get(&app, ITEM).await, (StatusCode::OK, saved.clone()));
	assert_eq!(b.get(&app, "/api/v1/pricing").await.1, json!({"items": [saved.clone()]}));

	let (status, stale) = b.write(&app, Method::PUT, ITEM, put).await;
	assert_eq!((status, stale), (StatusCode::CONFLICT, json!({"error": "stale", "current": saved})));

	let mut cheaper = cleaning.clone();
	cheaper["minimumCents"] = json!(3900);
	let (status, next) = b.write(&app, Method::PUT, ITEM, json!({"model": cheaper, "expected_updated_at": updated_at})).await;
	assert_eq!(status, StatusCode::OK, "{next}");
	let (status, _) = b.write(&app, Method::DELETE, ITEM, json!({"expected_updated_at": updated_at})).await;
	assert_eq!(status, StatusCode::CONFLICT, "a removal names the version it removes");
	assert_eq!(b.write(&app, Method::DELETE, ITEM, json!({})).await.0, StatusCode::BAD_REQUEST);
	let (status, removed) = b.write(&app, Method::DELETE, ITEM, json!({"expected_updated_at": next["updated_at"]})).await;
	assert_eq!(status, StatusCode::OK, "{removed}");
	assert_eq!((removed["model"].clone(), removed["updated_by"].clone()), (Value::Null, json!("dev-admin@localhost")));
	assert_eq!(site.get(&app, LIVE).await, (StatusCode::OK, json!({})), "back to the baked model");

	let (status, history) = b.get(&app, &format!("{ITEM}/changes")).await;
	assert_eq!(status, StatusCode::OK);
	let summary: Vec<(&str, &Value, &Value, &str)> = history["changes"]
		.as_array()
		.unwrap()
		.iter()
		.map(|c| (c["kind"].as_str().unwrap(), &c["valid_from"], &c["needs"], c["by"].as_str().unwrap()))
		.collect();
	assert_eq!(
		summary,
		[
			("remove", &Value::Null, &Value::Null, "dev-admin@localhost"),
			("set", &json!("2026-10-01"), &json!(2), "dev-admin@localhost"),
			("set", &json!("2026-10-01"), &json!(2), "dev-admin@localhost"),
		]
	);

	// Without the sign-in configured, the sites still read.
	assert_eq!(Browser::default().get(&http::router(panel), LIVE).await, (StatusCode::OK, json!({})));
}

/// The editor's preview is what the site shows: kitstart's cases, priced over HTTP, to the cent.
#[tokio::test]
async fn the_preview_prices_kitstarts_cases() {
	let db = TestDb::create().await;
	let app = app(panel(&db).await, Role::Operator);
	let mut b = Browser::signed_in(&app).await;
	let Value::Array(cases) = fixture("cases.json") else { panic!("a list") };
	let cleaning: Vec<&Value> = cases.iter().filter(|c| c["name"].as_str().unwrap().starts_with("cleaning:")).collect();
	assert!(cleaning.len() >= 3);
	for case in cleaning {
		let body = json!({"model": case["model"], "need": case["need"], "inputs": case["inputs"]});
		let (status, answer) = b.write(&app, Method::POST, "/api/v1/pricing/vifnet/preview", body).await;
		assert_eq!((status, &answer["cents"]), (StatusCode::OK, &case["cents"]), "{}", case["name"]);
	}
	let mut draft = cases[0]["model"].clone();
	draft["roundToCents"] = json!(0);
	let (status, answer) = b
		.write(&app, Method::POST, "/api/v1/pricing/vifnet/preview", json!({"model": draft, "need": "standard", "inputs": {}}))
		.await;
	assert_eq!(
		(status, answer),
		(StatusCode::UNPROCESSABLE_ENTITY, json!({"error": "an integer from 1 to 100000000", "path": "roundToCents"}))
	);
	let (status, _) = b.send(&app, Method::POST, "/api/v1/pricing/vifnet/preview", Some(json!({"model": {}, "need": "x"})), false).await;
	assert_eq!(status, StatusCode::FORBIDDEN, "a POST, so the CSRF header like any write");
}

/// A store that fails is a site that keeps its baked model: `{}`, never a 5xx.
#[tokio::test]
async fn the_sites_read_survives_a_failing_store() {
	let db = TestDb::create().await;
	let panel = panel(&db).await;
	let pool = db.pool().await;
	sqlx::query("DROP TABLE pricing_changes").execute(&pool).await.unwrap();
	sqlx::query("DROP TABLE pricing").execute(&pool).await.unwrap();
	assert_eq!(Browser::default().get(&http::router(panel), LIVE).await, (StatusCode::OK, json!({})));
}
