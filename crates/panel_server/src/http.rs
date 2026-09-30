//! The HTTP API. Only ingest for now, and `/health`.
//!
//! `POST /api/ingest/v1/events` is public and authenticated by the source's signature
//! alone (see `panel_core::signature`): no cookie, no CSRF, nothing but the key id and the
//! MAC over the raw body. The handler only translates; what is accepted and why is the
//! engine's.
//!
//! Answers: `207` with a verdict per event (protojson `sa.v1.IngestResponse`); `401` for a
//! batch whose key or signature is refused (the log says which, the caller only that it
//! was); `400` for a body that is not a batch; `413` past [`MAX_BODY`]; `500` for our own
//! failures, reported.

use axum::{
	Json, Router,
	body::Bytes,
	extract::{DefaultBodyLimit, State},
	http::{HeaderMap, StatusCode},
	response::{IntoResponse, Response},
	routing::{get, post},
};
use panel::{IngestError, Outcome, Panel, SignedBatch};
use panel_contracts::v1::{EventResult, IngestResponse};
use serde_json::json;

/// 500 events of about 8 KiB each: a batch is refused well before it strains anything.
pub const MAX_BODY: usize = 4 * 1024 * 1024;

pub fn router(panel: Panel) -> Router {
	Router::new()
		.route("/health", get(|| async { "ok" }))
		.route("/api/ingest/v1/events", post(ingest).layer(DefaultBodyLimit::max(MAX_BODY)))
		.with_state(panel)
}

fn error(status: StatusCode, msg: impl Into<String>) -> Response {
	(status, Json(json!({ "error": msg.into() }))).into_response()
}

fn header<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
	headers.get(name).and_then(|v| v.to_str().ok()).filter(|v| !v.is_empty())
}

/// `Bytes` stays the last extractor: the signature is over the body exactly as it came.
async fn ingest(State(panel): State<Panel>, headers: HeaderMap, body: Bytes) -> Response {
	let (Some(key_id), Some(timestamp), Some(signature)) = (header(&headers, "x-sa-key-id"), header(&headers, "x-sa-timestamp"), header(&headers, "x-sa-signature")) else {
		return error(StatusCode::UNAUTHORIZED, "x-sa-key-id, x-sa-timestamp and x-sa-signature are required");
	};
	let batch = SignedBatch {
		key_id,
		timestamp,
		signature,
		body: &body,
	};
	match panel.ingest(batch, jiff::Timestamp::now()).await {
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
		// a stale timestamp is only ever reported under a valid signature, so saying so
		// helps a source with a drifting clock and tells nobody else anything.
		Err(IngestError::Unauthorized(why)) => error(
			StatusCode::UNAUTHORIZED,
			if why == panel::STALE {
				"timestamp outside the replay window"
			} else {
				"invalid key or signature"
			},
		),
		Err(IngestError::BadRequest(e)) => error(StatusCode::BAD_REQUEST, e.0),
		Err(IngestError::Internal(e)) => {
			crate::report(&e, "ingest failed");
			error(StatusCode::INTERNAL_SERVER_ERROR, "internal error")
		}
	}
}
