//! The HTTP API: ingest, `/health`, and — when signing in is configured — the sign-in
//! (`/auth/*`, [`crate::signin`]) and the operator API (`/api/v1/*`, [`crate::api`]).
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

use std::{sync::Arc, time::Duration};

use axum::{
	Json, Router,
	body::{Body, to_bytes},
	error_handling::HandleErrorLayer,
	extract::State,
	http::{HeaderMap, HeaderValue, StatusCode, header},
	middleware,
	response::{IntoResponse, Response},
	routing::{get, post},
};
use panel::{IngestError, Outcome, Panel, SignedBatch};
use panel_contracts::v1::{EventResult, IngestResponse};
use panel_core::signature::{self, SignatureError};
use serde_json::json;
use tower::{BoxError, ServiceBuilder, limit::GlobalConcurrencyLimitLayer};
use tower_http::timeout::{RequestBodyTimeoutLayer, TimeoutError, TimeoutLayer};

use crate::{
	api,
	signin::{self, SignIn},
	telegram::{self, BotName, TelegramState},
};

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
	/// `/auth/*` requests at once: each may wait on concierge, so few.
	pub auth_concurrent: usize,
	/// `/auth/*`: a code exchange and a session write, concierge's 5 s included.
	pub auth_timeout: Duration,
	/// `/api/v1` requests at once.
	pub api_concurrent: usize,
	/// `/api/v1`: a rotation (which may wait on another replica's) and a few queries.
	pub api_timeout: Duration,
}

impl Default for Limits {
	fn default() -> Self {
		Self {
			max_concurrent: 32,
			body_timeout: Duration::from_secs(10),
			request_timeout: Duration::from_secs(30),
			auth_concurrent: 8,
			auth_timeout: Duration::from_secs(10),
			api_concurrent: 32,
			api_timeout: Duration::from_secs(15),
		}
	}
}

pub fn router(panel: Panel) -> Router {
	router_with(panel, Limits::default())
}

/// Ingest, and the sign-in with the operator API behind it.
pub fn app(sign_in: SignIn) -> Router {
	app_with(sign_in, Limits::default())
}

pub fn app_with(sign_in: SignIn, limits: Limits) -> Router {
	app_with_telegram(sign_in, limits, BotName::off())
}

/// [`app_with`], with the profile's Telegram routes knowing the bot.
pub fn app_with_telegram(sign_in: SignIn, limits: Limits, bot: BotName) -> Router {
	let auth = Router::new()
		.route("/auth/login", get(signin::login))
		.route("/auth/callback", get(signin::callback))
		.route("/auth/logout", post(signin::logout));
	let auth = bounded(auth, limits.auth_concurrent, limits.auth_timeout).with_state(sign_in.clone());
	let reads_and_edits = api::routes().route_layer(middleware::from_fn_with_state(sign_in.clone(), signin::gate));
	// Minting and revoking source keys asks concierge afresh: a grant revoked a moment ago
	// must not still mint a key from the cache.
	let key_changes = api::key_changes().route_layer(middleware::from_fn_with_state(sign_in.clone(), signin::gate_fresh));
	let telegram = telegram::routes()
		.route_layer(middleware::from_fn_with_state(sign_in.clone(), signin::gate))
		.with_state(TelegramState { panel: sign_in.panel.clone(), bot });
	let api = bounded(
		reads_and_edits.merge(key_changes).with_state(sign_in.panel.clone()).merge(telegram),
		limits.api_concurrent,
		limits.api_timeout,
	);
	router_with(sign_in.panel, limits).merge(auth).nest("/api/v1", api).layer(middleware::map_response(nosniff))
}

/// At most `concurrent` at once across `routes`, past it `503` rather than a queue; at most
/// `timeout` each. The semaphore is shared: `Router::layer` wraps every route on its own, and
/// a plain concurrency limit would give each route a budget of its own.
fn bounded<S: Clone + Send + Sync + 'static>(routes: Router<S>, concurrent: usize, timeout: Duration) -> Router<S> {
	let permits = Arc::new(tokio::sync::Semaphore::new(concurrent));
	routes.layer(
		ServiceBuilder::new()
			// The only error the layers below raise is the shed: everything else is a response.
			.layer(HandleErrorLayer::new(|_: BoxError| async { error(StatusCode::SERVICE_UNAVAILABLE, "busy, try again") }))
			.load_shed()
			.layer(GlobalConcurrencyLimitLayer::with_semaphore(permits))
			.layer(TimeoutLayer::with_status_code(StatusCode::SERVICE_UNAVAILABLE, timeout)),
	)
}

/// Every answer is what its `Content-Type` says, never sniffed into something else.
async fn nosniff(mut res: Response) -> Response {
	res.headers_mut().insert(header::X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff"));
	res
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
		.layer(middleware::map_response(nosniff))
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
