//! A place's live settings over HTTP (`panel::place`): the editor's routes under `/api/v1`
//! behind the sign-in, and the sites' read under `/api/internal`, which has no session.
//!
//! ```text
//! GET  /api/v1/places/{brand}/{slug}/settings                 {brand, slug, withdrawn, settings,
//!                                                              updated_at, updated_by, can_edit}
//! PUT  /api/v1/places/{brand}/{slug}/settings        admin    {settings, expected_updated_at}
//!                                                              → 200 as GET; 409 stale; 422 fields
//! GET  /api/v1/places/{brand}/{slug}/settings/history         {changes: [{id, at, by, kind, before, after}]}
//! POST /api/v1/places/{brand}/{slug}/settings/revert/{id}  admin  → 200 as GET; 404
//! POST /api/v1/places/{brand}/{slug}/withdraw | /restore   admin  → 200 as GET
//! POST /api/v1/places                                admin    {brand, slug} → 201 as GET (200 if known)
//! GET  /api/internal/brands/{brand}/locations/{slug}?locale    PlaceLive, `{}` when none or
//!                                                              unknown; 404 only when withdrawn
//! ```

use std::time::Duration;

use axum::{
	Extension, Json, Router,
	error_handling::HandleErrorLayer,
	extract::{Path, State, rejection::JsonRejection},
	http::StatusCode,
	response::{IntoResponse, Response},
	routing::{get, post, put},
};
use jiff::Timestamp;
use panel::{
	Panel,
	place::{Expected, Live, PlaceError, PlaceView},
};
use panel_core::{
	ids::{BrandId, LocationId},
	place::{Editor, PlaceSettings},
};
use serde::Deserialize;
use serde_json::{Value, json};
use tower::{BoxError, ServiceBuilder};
use tower_http::timeout::TimeoutLayer;
use uuid::Uuid;

use crate::{api::ApiError, signin::Caller};

/// The editor's reads: behind the gate, every role.
pub fn reads() -> Router<Panel> {
	Router::new()
		.route("/places/{brand}/{slug}/settings", get(settings))
		.route("/places/{brand}/{slug}/settings/history", get(history))
}

/// The editor's writes, an admin's: served behind `gate_fresh`, so a grant revoked a moment
/// ago cannot still move a phone number.
pub fn writes() -> Router<Panel> {
	Router::new()
		.route("/places", post(register))
		.route("/places/{brand}/{slug}/settings", put(set))
		.route("/places/{brand}/{slug}/settings/revert/{id}", post(revert))
		.route("/places/{brand}/{slug}/withdraw", post(withdraw))
		.route("/places/{brand}/{slug}/restore", post(restore))
}

/// The sites' read, mounted whether or not signing in is configured. Bounded like ingest; a
/// site answered 503 serves its baked place, so shedding costs a site nothing.
pub fn internal() -> Router<Panel> {
	let layers = ServiceBuilder::new()
		.layer(HandleErrorLayer::new(|_: BoxError| async { (StatusCode::SERVICE_UNAVAILABLE, Json(json!({ "error": "busy" }))) }))
		.load_shed()
		.concurrency_limit(32)
		.layer(TimeoutLayer::with_status_code(StatusCode::SERVICE_UNAVAILABLE, Duration::from_secs(3)));
	Router::new().route("/api/internal/brands/{brand}/locations/{slug}", get(live).layer(layers))
}

/// Why a change to a place failed, in the contract's shapes.
enum PlaceApiError {
	Api(ApiError),
	Conflict,
	Invalid(panel_core::place::FieldErrors),
}

impl IntoResponse for PlaceApiError {
	fn into_response(self) -> Response {
		match self {
			Self::Api(e) => e.into_response(),
			Self::Conflict => (StatusCode::CONFLICT, Json(json!({ "error": "conflict" }))).into_response(),
			Self::Invalid(fields) => (StatusCode::UNPROCESSABLE_ENTITY, Json(json!({ "error": "invalid", "fields": fields }))).into_response(),
		}
	}
}

impl From<ApiError> for PlaceApiError {
	fn from(e: ApiError) -> Self {
		Self::Api(e)
	}
}

impl From<eyre::Report> for PlaceApiError {
	fn from(e: eyre::Report) -> Self {
		Self::Api(ApiError::Internal(e))
	}
}

fn place_error(e: PlaceError) -> PlaceApiError {
	match e {
		PlaceError::NotFound => PlaceApiError::Api(ApiError::NotFound),
		PlaceError::Conflict => PlaceApiError::Conflict,
		PlaceError::Invalid(f) => PlaceApiError::Invalid(f),
		PlaceError::Internal(e) => PlaceApiError::Api(ApiError::Internal(e)),
	}
}

type PlaceResult<T> = Result<T, PlaceApiError>;

fn ids(brand: &str, slug: &str) -> Result<(BrandId, LocationId), ApiError> {
	Ok((BrandId::parse(brand)?, LocationId::parse(slug)?))
}

fn admin(caller: &Caller) -> Result<(), ApiError> {
	if caller.role.edits_places() { Ok(()) } else { Err(ApiError::Forbidden) }
}

/// Who the history names: the user's email, their id when concierge gave none.
fn editor(caller: &Caller) -> Editor {
	let label = if caller.email.trim().is_empty() { caller.user_id.to_string() } else { caller.email.clone() };
	Editor::User { id: caller.user_id, label }
}

fn body(v: PlaceView, caller: &Caller) -> Value {
	json!({
		"brand": v.brand.as_str(),
		"slug": v.slug.as_str(),
		"withdrawn": v.withdrawn,
		"settings": v.settings.as_json(),
		"updated_at": v.updated_at.map(|t| t.to_string()),
		"updated_by": v.updated_by,
		"can_edit": caller.role.edits_places(),
	})
}

async fn settings(State(panel): State<Panel>, Extension(caller): Extension<Caller>, Path((brand, slug)): Path<(String, String)>) -> PlaceResult<Json<Value>> {
	let (brand, slug) = ids(&brand, &slug)?;
	Ok(Json(body(panel.place(&brand, &slug).await?, &caller)))
}

async fn history(State(panel): State<Panel>, Path((brand, slug)): Path<(String, String)>) -> PlaceResult<Json<Value>> {
	let (brand, slug) = ids(&brand, &slug)?;
	let changes: Vec<Value> = panel
		.place_history(&brand, &slug)
		.await?
		.into_iter()
		.map(|c| {
			json!({
				"id": c.id.to_string(),
				"at": c.at.to_string(),
				"by": c.by,
				"kind": c.kind.as_str(),
				"before": c.before.as_json(),
				"after": c.after.as_json(),
				"reverts": c.reverts.map(|r| r.to_string()),
			})
		})
		.collect();
	Ok(Json(json!({ "changes": changes })))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SetBody {
	settings: Value,
	/// The `updated_at` the editor read; null (or absent) for settings never set.
	#[serde(default)]
	expected_updated_at: Option<String>,
}

fn json_body<T: serde::de::DeserializeOwned>(b: Result<Json<T>, JsonRejection>) -> Result<T, ApiError> {
	b.map(|Json(v)| v).map_err(|e| ApiError::BadRequest(e.body_text()))
}

async fn set(
	State(panel): State<Panel>,
	Extension(caller): Extension<Caller>,
	Path((brand, slug)): Path<(String, String)>,
	b: Result<Json<SetBody>, JsonRejection>,
) -> PlaceResult<Json<Value>> {
	admin(&caller)?;
	let (brand, slug) = ids(&brand, &slug)?;
	let b = json_body(b)?;
	let expected = match b.expected_updated_at.as_deref() {
		None => None,
		Some(raw) => Some(
			raw.parse::<Timestamp>()
				.map_err(|_| ApiError::BadRequest("expected_updated_at is not an RFC 3339 instant".into()))?,
		),
	};
	let settings = PlaceSettings::parse(&b.settings).map_err(PlaceApiError::Invalid)?;
	let v = panel
		.set_place(&editor(&caller), &brand, &slug, settings, Expected::At(expected), Timestamp::now())
		.await
		.map_err(place_error)?;
	Ok(Json(body(v, &caller)))
}

async fn revert(State(panel): State<Panel>, Extension(caller): Extension<Caller>, Path((brand, slug, id)): Path<(String, String, String)>) -> PlaceResult<Json<Value>> {
	admin(&caller)?;
	let (brand, slug) = ids(&brand, &slug)?;
	let id = Uuid::parse_str(&id).map_err(|_| ApiError::NotFound)?;
	let v = panel
		.revert_place(&editor(&caller), &brand, &slug, id, Expected::Any, Timestamp::now())
		.await
		.map_err(place_error)?;
	Ok(Json(body(v, &caller)))
}

async fn withdrawal(panel: &Panel, caller: &Caller, brand: &str, slug: &str, withdrawn: bool) -> PlaceResult<Json<Value>> {
	admin(caller)?;
	let (brand, slug) = ids(brand, slug)?;
	let v = panel.withdraw_place(&editor(caller), &brand, &slug, withdrawn, Timestamp::now()).await.map_err(place_error)?;
	Ok(Json(body(v, caller)))
}

async fn withdraw(State(panel): State<Panel>, Extension(caller): Extension<Caller>, Path((brand, slug)): Path<(String, String)>) -> PlaceResult<Json<Value>> {
	withdrawal(&panel, &caller, &brand, &slug, true).await
}

async fn restore(State(panel): State<Panel>, Extension(caller): Extension<Caller>, Path((brand, slug)): Path<(String, String)>) -> PlaceResult<Json<Value>> {
	withdrawal(&panel, &caller, &brand, &slug, false).await
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RegisterBody {
	brand: String,
	slug: String,
}

async fn register(State(panel): State<Panel>, Extension(caller): Extension<Caller>, b: Result<Json<RegisterBody>, JsonRejection>) -> PlaceResult<Response> {
	admin(&caller)?;
	let b = json_body(b)?;
	let (brand, slug) = ids(&b.brand, &b.slug)?;
	let (v, added) = panel.register_place(&editor(&caller), &brand, &slug, Timestamp::now()).await?;
	let status = if added { StatusCode::CREATED } else { StatusCode::OK };
	Ok((status, Json(body(v, &caller))).into_response())
}

/// What a site gets. `locale` is accepted and not needed: a landmark is stored with every
/// locale's text, and the site picks its own.
async fn live(State(panel): State<Panel>, Path((brand, slug)): Path<(String, String)>) -> Response {
	// A brand or slug that could not name a place is a place the panel does not know: `{}`,
	// never the 404 that would take a live page down.
	let (Ok(brand), Ok(slug)) = (BrandId::parse(&brand), LocationId::parse(&slug)) else {
		return Json(json!({})).into_response();
	};
	match panel.live_place(&brand, &slug).await {
		Ok(Live::Settings(s)) => Json(s.as_json()).into_response(),
		Ok(Live::Withdrawn) => (StatusCode::NOT_FOUND, Json(json!({ "error": "not_found" }))).into_response(),
		Err(e) => {
			// The site keeps its baked place on a 5xx: an outage here delays a change, no more.
			crate::report(&e, "serving a place to a site");
			(StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": "internal error" }))).into_response()
		}
	}
}
