//! The HTTP ingest end to end, through the real router, against a real Postgres.

use axum::{
	body::{Body, to_bytes},
	http::{Request, StatusCode},
};
use jiff::Timestamp;
use panel::testing::{Signed, TestDb, event, panel, sign};
use panel_core::{event::SourceKind, ids::BrandId};
use serde_json::{Value, json};
use tower::ServiceExt;

async fn send(app: axum::Router, signed: &Signed, headers: &[&str]) -> (StatusCode, Value) {
	let mut req = Request::post("/api/ingest/v1/events").header("content-type", "application/json");
	for h in headers {
		req = match *h {
			"x-sa-key-id" => req.header(*h, &signed.key_id),
			"x-sa-timestamp" => req.header(*h, &signed.timestamp),
			"x-sa-signature" => req.header(*h, &signed.signature),
			other => panic!("{other}"),
		};
	}
	let res = app.oneshot(req.body(Body::from(signed.body.clone())).unwrap()).await.unwrap();
	let status = res.status();
	let body = to_bytes(res.into_body(), usize::MAX).await.unwrap();
	(status, serde_json::from_slice(&body).unwrap_or(Value::Null))
}

const ALL: [&str; 3] = ["x-sa-key-id", "x-sa-timestamp", "x-sa-signature"];

#[tokio::test]
async fn ingest_over_http() {
	let Some(db) = TestDb::create().await else { return };
	let panel = panel(&db).await;
	let secret = panel
		.add_source("aquafix-site", SourceKind::Site, [BrandId::parse("aquafix").unwrap()].into())
		.await
		.unwrap()
		.unwrap()
		.secret
		.to_string();
	let app = panel_server::http::router(panel);
	let now = Timestamp::now();
	let lead = json!({"brandId": "aquafix", "leadId": "L-1"});
	let created = event("lead.created", now, "site", lead.clone(), json!({"channel": "form"}));
	let events = [
		created.clone(),
		created,
		event("lead.created", now, "site", json!({"brandId": "vifnet", "leadId": "V-1"}), json!({"channel": "form"})),
		event("review.new", now, "site", lead, json!({})),
	];

	let good = sign("aquafix-site", &secret, &events, now);
	let (status, body) = send(app.clone(), &good, &ALL).await;
	assert_eq!(status, StatusCode::MULTI_STATUS, "{body}");
	let results = body["results"].as_array().unwrap();
	let statuses: Vec<&str> = results.iter().map(|r| r["status"].as_str().unwrap()).collect();
	assert_eq!(statuses, ["accepted", "duplicate", "rejected", "accepted"]);
	assert_eq!(results[2]["reason"], "this key may not write for brand vifnet");
	assert_eq!(results[2]["index"], 2);
	assert_eq!(results[0]["id"], events[0]["id"]);
	assert!(results[3]["reason"].as_str().unwrap().contains("not registered"));

	let (status, body) = send(app.clone(), &good, &ALL[..2]).await;
	assert_eq!(status, StatusCode::UNAUTHORIZED, "{body}");

	let forged = sign("aquafix-site", &"f".repeat(64), &events, now);
	let unknown = sign("nobody", &secret, &events, now);
	let (s1, b1) = send(app.clone(), &forged, &ALL).await;
	let (s2, b2) = send(app.clone(), &unknown, &ALL).await;
	assert_eq!((s1, s2), (StatusCode::UNAUTHORIZED, StatusCode::UNAUTHORIZED));
	assert_eq!(b1, b2, "an unknown key and a bad signature read the same");

	let stale = sign("aquafix-site", &secret, &events, now - jiff::SignedDuration::from_mins(10));
	let (status, body) = send(app.clone(), &stale, &ALL).await;
	assert_eq!((status, body["error"].as_str()), (StatusCode::UNAUTHORIZED, Some("timestamp outside the replay window")));

	let empty = sign("aquafix-site", &secret, &[], now);
	let (status, body) = send(app.clone(), &empty, &ALL).await;
	assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");

	let huge = Signed {
		body: vec![b' '; panel_server::http::MAX_BODY + 1],
		..sign("aquafix-site", &secret, &events, now)
	};
	let (status, _) = send(app.clone(), &huge, &ALL).await;
	assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);

	let res = app.oneshot(Request::get("/health").body(Body::empty()).unwrap()).await.unwrap();
	assert_eq!(res.status(), StatusCode::OK);
}

/// A body that never arrives.
fn stalled() -> Body {
	Body::from_stream(futures::stream::pending::<Result<Vec<u8>, std::io::Error>>())
}

fn request(key_id: &str, timestamp: &str, body: Body) -> Request<Body> {
	Request::post("/api/ingest/v1/events")
		.header("x-sa-key-id", key_id)
		.header("x-sa-timestamp", timestamp)
		.header("x-sa-signature", "00")
		.body(body)
		.unwrap()
}

#[tokio::test]
async fn costly_requests_are_refused_before_the_body_is_read() {
	let Some(db) = TestDb::create().await else { return };
	let panel = panel(&db).await;
	let now = Timestamp::now().as_second().to_string();
	let stale = (Timestamp::now().as_second() - 600).to_string();
	let quick = std::time::Duration::from_secs(5);

	// Both would hang on the stalled body if it were read first.
	let app = panel_server::http::router(panel.clone());
	for (key_id, ts) in [("aquafix-site", stale.as_str()), ("Not a key id", now.as_str())] {
		let res = tokio::time::timeout(quick, app.clone().oneshot(request(key_id, ts, stalled())))
			.await
			.expect("answered without the body")
			.unwrap();
		assert_eq!(res.status(), StatusCode::UNAUTHORIZED, "{key_id} {ts}");
	}

	let limits = panel_server::http::Limits {
		max_concurrent: 1,
		body_timeout: std::time::Duration::from_millis(200),
		request_timeout: std::time::Duration::from_secs(5),
	};
	let app = panel_server::http::router_with(panel, limits);
	let res = tokio::time::timeout(quick, app.clone().oneshot(request("aquafix-site", &now, stalled()))).await.unwrap().unwrap();
	assert_eq!(res.status(), StatusCode::REQUEST_TIMEOUT, "a body too slow to arrive");

	// One slow request holds the only slot; the next is shed, not queued.
	let (tx, rx) = tokio::sync::oneshot::channel::<()>();
	let held = futures::stream::once(async move {
		let _ = rx.await;
		Ok::<_, std::io::Error>(b"{}".to_vec())
	});
	let first = tokio::spawn(app.clone().oneshot(request("aquafix-site", &now, Body::from_stream(held))));
	tokio::time::sleep(std::time::Duration::from_millis(50)).await;
	let res = app.clone().oneshot(request("aquafix-site", &now, Body::from("{}"))).await.unwrap();
	assert_eq!(res.status(), StatusCode::SERVICE_UNAVAILABLE);
	tx.send(()).unwrap();
	assert_eq!(first.await.unwrap().unwrap().status(), StatusCode::UNAUTHORIZED, "the held one goes on to be judged");
}
