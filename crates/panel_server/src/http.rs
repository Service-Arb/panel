//! The HTTP API. Only ingest for now, and `/health`.
//!
//! `POST /api/ingest/v1/events` is authenticated by the source's signature alone (see
//! `panel_core::signature`): no cookie, no CSRF, nothing but the key id and the MAC over the
//! raw body. It is meant for the cluster's own sources and is not published by the ingress
//! (docs/ARCHITECTURE.md, Deploy). The handler only translates; what is accepted and why is
//! the engine's.
//!
//! Before a byte of the body is read, the headers are checked for what costs nothing — the
//! key id's shape, the timestamp's window — and the body is then read up to [`MAX_BODY`]
//! only. The route sheds load past a number of requests at once, and bounds how long a body
//! may take to arrive and a request in all ([`Limits`]).
//!
//! Answers: `207` with a verdict per event (protojson `sa.v1.IngestResponse`); `401` for a
//! batch whose key or signature is refused; `400` for a body that is not a batch; `408` for
//! a body too slow to arrive; `413` past [`MAX_BODY`]; `503` when shedding load; `500` for
//! our own failures, reported.

use std::time::Duration;

use axum::{
	Json, Router,
	body::{Body, to_bytes},
	error_handling::HandleErrorLayer,
	extract::State,
	http::{HeaderMap, StatusCode},
	response::{IntoResponse, Response},
	routing::{get, post},
};
use panel::{IngestError, Outcome, Panel, SignedBatch};
use panel_contracts::v1::{EventResult, IngestResponse};
use panel_core::signature::{self, SignatureError};
use serde_json::json;
use tower::{BoxError, ServiceBuilder};
use tower_http::timeout::{RequestBodyTimeoutLayer, TimeoutError, TimeoutLayer};

/// 500 events of about 8 KiB each: a batch is refused well before it strains anything.
pub const MAX_BODY: usize = 4 * 1024 * 1024;

/// What one ingest request may cost.
#[derive(Clone, Copy, Debug)]
pub struct Limits {
	/// Ingest requests served at once; past it, `503` rather than a queue.
	pub max_concurrent: usize,
	/// How long a body may take to arrive.
	pub body_timeout: Duration,
	/// How long a request may take in all: 500 short transactions, with room to spare.
	pub request_timeout: Duration,
}

impl Default for Limits {
	fn default() -> Self {
		Self {
			max_concurrent: 32,
			body_timeout: Duration::from_secs(10),
			request_timeout: Duration::from_secs(30),
		}
	}
}

pub fn router(panel: Panel) -> Router {
	router_with(panel, Limits::default())
}

pub fn router_with(panel: Panel, limits: Limits) -> Router {
	let layers = ServiceBuilder::new()
		// The only error the layers below raise is the shed: everything else is a response.
		.layer(HandleErrorLayer::new(|_: BoxError| async { error(StatusCode::SERVICE_UNAVAILABLE, "busy, try again") }))
		.load_shed()
		.concurrency_limit(limits.max_concurrent)
		.layer(TimeoutLayer::with_status_code(StatusCode::REQUEST_TIMEOUT, limits.request_timeout))
		.layer(RequestBodyTimeoutLayer::new(limits.body_timeout));
	Router::new()
		.route("/health", get(|| async { "ok" }))
		.route("/api/ingest/v1/events", post(ingest).layer(layers))
		.with_state(panel)
}

fn error(status: StatusCode, msg: impl Into<String>) -> Response {
	(status, Json(json!({ "error": msg.into() }))).into_response()
}

fn header<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
	headers.get(name).and_then(|v| v.to_str().ok()).filter(|v| !v.is_empty())
}

const STALE_MESSAGE: &str = "timestamp outside the replay window";
const REFUSED_MESSAGE: &str = "invalid key or signature";

/// What went wrong reading a body: too big, too slow, or cut off.
fn body_error(e: axum::Error) -> Response {
	let inner = e.into_inner();
	let mut cause: Option<&(dyn std::error::Error + 'static)> = Some(inner.as_ref());
	while let Some(c) = cause {
		if c.is::<http_body_util::LengthLimitError>() {
			return error(StatusCode::PAYLOAD_TOO_LARGE, format!("the body is larger than {MAX_BODY} bytes"));
		}
		if c.is::<TimeoutError>() {
			return error(StatusCode::REQUEST_TIMEOUT, "the body took too long to arrive");
		}
		cause = c.source();
	}
	error(StatusCode::BAD_REQUEST, "the body could not be read")
}

/// `Body` stays the last extractor and is read by hand: the signature is over the body
/// exactly as it came, and nothing is read before the headers pass.
async fn ingest(State(panel): State<Panel>, headers: HeaderMap, body: Body) -> Response {
	let (Some(key_id), Some(timestamp), Some(signature)) = (header(&headers, "x-sa-key-id"), header(&headers, "x-sa-timestamp"), header(&headers, "x-sa-signature")) else {
		return error(StatusCode::UNAUTHORIZED, "x-sa-key-id, x-sa-timestamp and x-sa-signature are required");
	};
	let now = jiff::Timestamp::now();
	if let Err(e @ (SignatureError::Stale | SignatureError::MalformedTimestamp)) = signature::check_window(timestamp, now) {
		tracing::debug!(error = %e, "ingest: refused on the timestamp");
		return error(StatusCode::UNAUTHORIZED, STALE_MESSAGE);
	}
	if !panel_core::ids::is_slug(key_id) {
		return error(StatusCode::UNAUTHORIZED, REFUSED_MESSAGE);
	}
	let body = match to_bytes(body, MAX_BODY).await {
		Ok(body) => body,
		Err(e) => return body_error(e),
	};
	let batch = SignedBatch {
		key_id,
		timestamp,
		signature,
		body: &body,
	};
	match panel.ingest(batch, now).await {
		Ok(verdicts) => {
			let results = verdicts
				.into_iter()
				.map(|v| {
					let (status, reason) = match v.outcome {
						Outcome::Accepted { unregistered: false } => ("accepted", None),
						Outcome::Accepted { unregistered: true } => ("accepted", Some("type not registered: stored, not projected".to_owned())),
						Outcome::Duplicate => ("duplicate", None),
						Outcome::Rejected(e) => ("rejected", Some(e.0)),
					};
					EventResult {
						// `wire::MAX_BATCH` keeps this far below u32::MAX.
						index: u32::try_from(v.index).unwrap_or(u32::MAX),
						id: v.id,
						status: status.to_owned(),
						reason,
					}
				})
				.collect();
			(StatusCode::MULTI_STATUS, Json(IngestResponse { results })).into_response()
		}
		// An unknown key and a bad signature read the same, so key ids cannot be probed for;
		// only a timestamp outside the window is told apart, and it says nothing about keys.
		Err(IngestError::Unauthorized(why)) => error(StatusCode::UNAUTHORIZED, if why == panel::STALE { STALE_MESSAGE } else { REFUSED_MESSAGE }),
		Err(IngestError::BadRequest(e)) => error(StatusCode::BAD_REQUEST, e.0),
		Err(IngestError::Internal(e)) => {
			crate::report(&e, "ingest failed");
			error(StatusCode::INTERNAL_SERVER_ERROR, "internal error")
		}
	}
}
