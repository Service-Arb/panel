//! The operator API, `/api/v1`, for the panel's front end. JSON in and out; every request has
//! passed [`crate::signin::gate`], so a [`Caller`] is in its extensions. What an action
//! means is the engine's (`panel::operator`); this only translates and asks the role.
//!
//! Timestamps are RFC 3339 strings, money is minor units of its currency.

use std::collections::BTreeSet;

use axum::{
	Extension, Json, Router,
	extract::{Path, Query, State, rejection::JsonRejection},
	http::{HeaderMap, HeaderValue, StatusCode, header},
	response::{IntoResponse, Response},
	routing::{delete, get, post},
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use jiff::{SignedDuration, Timestamp, civil::Date, tz::TimeZone};
use panel::{
	Panel,
	operator::{ActionError, Actor, CallOutcome, EventView, LeadQuery, LeadView, NewLead, Payment, Pii, StageMove},
};
use panel_core::{
	Invalid,
	event::SourceKind,
	funnel::{MIN_SAMPLE, Share},
	ids::{BrandId, JobId, LeadId, LocationId},
	lead::Stage,
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::signin::Caller;

/// The longest span the funnel sums over.
const MAX_FUNNEL_DAYS: i32 = 366;

/// The largest amount, in minor units, a quote or a payment may name: €100M, far past any
/// real job, well short of anything that overflows a sum.
const MAX_MINOR: i64 = 10_000_000_000;

/// The currencies a quote or a payment may be in. Checked here, not in the registry: a rebuild
/// must not re-judge what was journaled under an older list.
const CURRENCIES: [&str; 4] = ["EUR", "USD", "GBP", "AUD"];

/// The header a client sets to make a write safe to retry.
const IDEMPOTENCY_KEY: &str = "idempotency-key";

fn amount(what: &str, v: i64) -> ApiResult<i64> {
	if v.unsigned_abs() > MAX_MINOR.unsigned_abs() {
		return Err(ApiError::BadRequest(format!("{what} is more than {MAX_MINOR} minor units")));
	}
	Ok(v)
}

fn currency(v: &str) -> ApiResult<String> {
	if !CURRENCIES.contains(&v) {
		return Err(ApiError::BadRequest(format!("currency is one of {}", CURRENCIES.join(", "))));
	}
	Ok(v.to_owned())
}

/// The client's idempotency key: 1–128 visible ASCII characters, or none.
fn idempotency_key(headers: &HeaderMap) -> ApiResult<Option<String>> {
	let Some(v) = headers.get(IDEMPOTENCY_KEY) else { return Ok(None) };
	let v = v.to_str().ok().filter(|v| (1..=128).contains(&v.len()) && v.bytes().all(|b| b.is_ascii_graphic()));
	v.map(|v| Some(v.to_owned()))
		.ok_or_else(|| ApiError::BadRequest("Idempotency-Key is 1–128 visible ASCII characters".into()))
}

/// `201` for what was recorded now, `200` for a retry answered with what the first recorded.
fn recorded(replayed: bool, v: Value) -> Response {
	let status = if replayed { StatusCode::OK } else { StatusCode::CREATED };
	(status, Json(v)).into_response()
}

pub fn routes() -> Router<Panel> {
	Router::new()
		.route("/me", get(me))
		.route("/leads", get(leads).post(create_lead))
		.route("/leads/{brand}/{lead}", get(lead))
		.route("/leads/{brand}/{lead}/stage", post(stage))
		.route("/leads/{brand}/{lead}/calls/attempt", post(attempt_call))
		.route("/leads/{brand}/{lead}/calls/{attempt}/outcome", post(call_outcome))
		.route("/leads/{brand}/{lead}/payments", post(payment))
		.route("/funnel", get(funnel))
		.route("/sources", get(sources))
}

/// Minting and revoking source keys: served behind [`crate::signin::gate_fresh`].
pub fn key_changes() -> Router<Panel> {
	Router::new().route("/sources", post(add_source)).route("/sources/{key_id}", delete(revoke_source))
}

/// Why a request failed, as the front end is told.
#[derive(Debug)]
pub enum ApiError {
	BadRequest(String),
	Forbidden,
	NotFound,
	Conflict(String),
	Internal(eyre::Report),
}

impl IntoResponse for ApiError {
	fn into_response(self) -> Response {
		let (status, msg) = match self {
			Self::BadRequest(m) => (StatusCode::BAD_REQUEST, m),
			Self::Forbidden => (StatusCode::FORBIDDEN, "your role may not do this".to_owned()),
			Self::NotFound => (StatusCode::NOT_FOUND, "not found".to_owned()),
			Self::Conflict(m) => (StatusCode::CONFLICT, m),
			Self::Internal(e) => {
				crate::report(&e, "operator API");
				(StatusCode::INTERNAL_SERVER_ERROR, "internal error".to_owned())
			}
		};
		(status, Json(json!({ "error": msg }))).into_response()
	}
}

impl From<eyre::Report> for ApiError {
	fn from(e: eyre::Report) -> Self {
		Self::Internal(e)
	}
}

impl From<Invalid> for ApiError {
	fn from(e: Invalid) -> Self {
		Self::BadRequest(e.0)
	}
}

impl From<ActionError> for ApiError {
	fn from(e: ActionError) -> Self {
		match e {
			ActionError::NotFound => Self::NotFound,
			ActionError::Conflict(m) => Self::Conflict(m.to_owned()),
			ActionError::Invalid(e) => Self::BadRequest(e.0),
			ActionError::Internal(e) => Self::Internal(e),
		}
	}
}

type ApiResult<T> = Result<T, ApiError>;

/// A JSON body, its rejection turned into our error shape.
fn body<T: DeserializeOwned>(b: Result<Json<T>, JsonRejection>) -> ApiResult<T> {
	b.map(|Json(v)| v).map_err(|e| ApiError::BadRequest(e.body_text()))
}

fn allow(ok: bool) -> ApiResult<()> {
	if ok { Ok(()) } else { Err(ApiError::Forbidden) }
}

/// The point where the role decides whether PII is shown (§5.4).
fn pii(caller: &Caller) -> Pii {
	if caller.role.sees_pii() { Pii::Reveal } else { Pii::Withhold }
}

fn ids(brand: &str, lead: &str) -> ApiResult<(BrandId, LeadId)> {
	Ok((BrandId::parse(brand)?, LeadId::parse(lead)?))
}

fn ts(t: Option<Timestamp>) -> Option<String> {
	t.map(|t| t.to_string())
}

// ── /me ─────────────────────────────────────────────────────────────────────────────────

#[derive(Serialize)]
struct MeDto {
	user_id: String,
	role: &'static str,
	email: String,
	preferred_name: String,
}

async fn me(Extension(caller): Extension<Caller>) -> Json<MeDto> {
	Json(MeDto {
		user_id: caller.user_id.to_string(),
		role: caller.role.as_str(),
		email: caller.email,
		preferred_name: caller.preferred_name,
	})
}

// ── leads ───────────────────────────────────────────────────────────────────────────────

#[derive(Serialize)]
struct LeadDto {
	brand: String,
	lead_id: String,
	location: Option<String>,
	job_id: Option<String>,
	stage: &'static str,
	channel: Option<String>,
	manual: bool,
	created_at: Option<String>,
	contacted_at: Option<String>,
	quoted_at: Option<String>,
	won_at: Option<String>,
	completed_at: Option<String>,
	paid_at: Option<String>,
	lost_at: Option<String>,
	lost_reason: Option<String>,
	last_event_at: String,
	/// Set while it waits for its first contact.
	sla: Option<SlaDto>,
	/// What the customer left (name, phone, need, …), for the roles that see it; absent
	/// otherwise or when there is none.
	#[serde(skip_serializing_if = "Option::is_none")]
	pii: Option<Value>,
}

#[derive(Serialize)]
struct SlaDto {
	waiting_since: String,
	waiting_seconds: i64,
	overdue: bool,
}

fn lead_dto(v: LeadView, now: Timestamp) -> LeadDto {
	let r = v.row;
	LeadDto {
		sla: v.waiting.map(|w| SlaDto {
			waiting_since: w.since.to_string(),
			waiting_seconds: now.duration_since(w.since).as_secs(),
			overdue: w.overdue,
		}),
		pii: v.pii,
		brand: r.brand_id,
		lead_id: r.lead_id,
		location: r.location_id,
		job_id: r.job_id,
		stage: r.stage.as_str(),
		channel: r.channel,
		manual: r.manual,
		created_at: ts(r.created_at),
		contacted_at: ts(r.contacted_at),
		quoted_at: ts(r.quoted_at),
		won_at: ts(r.won_at),
		completed_at: ts(r.completed_at),
		paid_at: ts(r.paid_at),
		lost_at: ts(r.lost_at),
		lost_reason: r.lost_reason,
		last_event_at: r.last_event_at.to_string(),
	}
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LeadsQuery {
	stage: Option<String>,
	brand: Option<String>,
	location: Option<String>,
	overdue: Option<bool>,
	cursor: Option<String>,
	limit: Option<u32>,
}

fn cursor_encode((at, brand, lead): &(Timestamp, String, String)) -> String {
	URL_SAFE_NO_PAD.encode(format!("{at}|{brand}|{lead}"))
}

fn cursor_decode(raw: &str) -> ApiResult<(Timestamp, String, String)> {
	let bad = || ApiError::BadRequest("cursor is not one this API gave".into());
	let text = String::from_utf8(URL_SAFE_NO_PAD.decode(raw).map_err(|_| bad())?).map_err(|_| bad())?;
	let mut parts = text.splitn(3, '|');
	let (Some(at), Some(brand), Some(lead)) = (parts.next(), parts.next(), parts.next()) else {
		return Err(bad());
	};
	Ok((at.parse().map_err(|_| bad())?, brand.to_owned(), lead.to_owned()))
}

async fn leads(State(panel): State<Panel>, Extension(caller): Extension<Caller>, q: Result<Query<LeadsQuery>, axum::extract::rejection::QueryRejection>) -> ApiResult<Json<Value>> {
	let Query(q) = q.map_err(|e| ApiError::BadRequest(e.body_text()))?;
	let query = LeadQuery {
		stage: q.stage.as_deref().map(str::parse::<Stage>).transpose()?,
		brand: q.brand.as_deref().map(BrandId::parse).transpose()?,
		location: q.location.as_deref().map(LocationId::parse).transpose()?,
		overdue: q.overdue.unwrap_or(false),
		after: q.cursor.as_deref().map(cursor_decode).transpose()?,
		limit: q.limit.unwrap_or(50),
	};
	let now = Timestamp::now();
	let page = panel.leads(&query, pii(&caller), now).await?;
	let next = page.next.as_ref().map(cursor_encode);
	let leads: Vec<LeadDto> = page.leads.into_iter().map(|v| lead_dto(v, now)).collect();
	Ok(Json(json!({ "leads": leads, "next_cursor": next })))
}

#[derive(Serialize)]
struct EventDto {
	id: String,
	r#type: String,
	type_version: i32,
	occurred_at: String,
	received_at: String,
	source_kind: String,
	source_id: String,
	manual: bool,
	job_id: Option<String>,
	status: String,
	status_reason: Option<String>,
	properties: Value,
	#[serde(skip_serializing_if = "Option::is_none")]
	pii: Option<Value>,
}

fn event_dto(e: EventView) -> EventDto {
	let r = e.row;
	EventDto {
		manual: r.source_kind == SourceKind::Panel.as_str(),
		id: r.id.to_string(),
		r#type: r.r#type,
		type_version: r.type_version,
		occurred_at: r.occurred_at.to_string(),
		received_at: r.received_at.to_string(),
		source_kind: r.source_kind,
		source_id: r.source_id,
		job_id: r.job_id,
		status: r.status,
		status_reason: r.status_reason,
		properties: r.properties,
		pii: e.pii,
	}
}

async fn lead(State(panel): State<Panel>, Extension(caller): Extension<Caller>, Path((brand, lead)): Path<(String, String)>) -> ApiResult<Json<Value>> {
	let (brand, lead) = ids(&brand, &lead)?;
	let now = Timestamp::now();
	let (view, events) = panel.lead_card(&brand, &lead, pii(&caller), now).await?.ok_or(ApiError::NotFound)?;
	let events: Vec<EventDto> = events.into_iter().map(event_dto).collect();
	Ok(Json(json!({ "lead": lead_dto(view, now), "events": events })))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateLead {
	brand: String,
	location: String,
	need: String,
	phone: Option<String>,
}

async fn create_lead(State(panel): State<Panel>, Extension(caller): Extension<Caller>, headers: HeaderMap, b: Result<Json<CreateLead>, JsonRejection>) -> ApiResult<Response> {
	allow(caller.role.edits_leads())?;
	let key = idempotency_key(&headers)?;
	let b = body(b)?;
	let new = NewLead {
		brand: BrandId::parse(&b.brand)?,
		location: LocationId::parse(&b.location)?,
		need: b.need,
		phone: b.phone,
	};
	let brand = new.brand.clone();
	let done = panel.create_lead_once(Actor(caller.user_id), new, Timestamp::now(), key.as_deref()).await?;
	let (lead, event) = done.value;
	Ok(recorded(
		done.replayed,
		json!({ "brand": brand.as_str(), "lead_id": lead.as_str(), "event_id": event.raw().to_string() }),
	))
}

fn created(v: Value) -> Response {
	(StatusCode::CREATED, Json(v)).into_response()
}

#[derive(Deserialize)]
#[serde(tag = "stage", rename_all = "snake_case", deny_unknown_fields)]
enum StageBody {
	Contacted { channel: Option<String> },
	Quoted { amount: Option<i64>, currency: Option<String> },
	Won { job_id: Option<String> },
	Lost { reason: String, note: Option<String> },
	Completed,
}

async fn stage(
	State(panel): State<Panel>,
	Extension(caller): Extension<Caller>,
	Path((brand, lead)): Path<(String, String)>,
	headers: HeaderMap,
	b: Result<Json<StageBody>, JsonRejection>,
) -> ApiResult<Response> {
	allow(caller.role.edits_leads())?;
	let key = idempotency_key(&headers)?;
	let (brand, lead) = ids(&brand, &lead)?;
	let to = match body(b)? {
		StageBody::Contacted { channel } => StageMove::Contacted { channel },
		StageBody::Quoted { amount: a, currency: c } => StageMove::Quoted {
			amount: a.map(|a| amount("amount", a)).transpose()?,
			currency: c.as_deref().map(currency).transpose()?,
		},
		StageBody::Won { job_id } => StageMove::Won {
			job_id: job_id.as_deref().map(JobId::parse).transpose()?,
		},
		StageBody::Lost { reason, note } => StageMove::Lost { reason, note },
		StageBody::Completed => StageMove::Completed,
	};
	let done = panel.move_lead_once(Actor(caller.user_id), &brand, &lead, to, Timestamp::now(), key.as_deref()).await?;
	Ok(recorded(done.replayed, json!({ "event_id": done.value.raw().to_string() })))
}

async fn attempt_call(State(panel): State<Panel>, Extension(caller): Extension<Caller>, Path((brand, lead)): Path<(String, String)>) -> ApiResult<Response> {
	allow(caller.role.edits_leads())?;
	let (brand, lead) = ids(&brand, &lead)?;
	let attempt = panel.attempt_call(Actor(caller.user_id), &brand, &lead, Timestamp::now()).await?;
	Ok(created(json!({ "attempt_id": attempt.raw().to_string() })))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OutcomeBody {
	outcome: String,
}

async fn call_outcome(
	State(panel): State<Panel>,
	Extension(caller): Extension<Caller>,
	Path((brand, lead, attempt)): Path<(String, String, String)>,
	b: Result<Json<OutcomeBody>, JsonRejection>,
) -> ApiResult<Response> {
	allow(caller.role.edits_leads())?;
	let (brand, lead) = ids(&brand, &lead)?;
	let attempt = Uuid::parse_str(&attempt).map_err(|_| ApiError::NotFound)?;
	let call = CallOutcome { attempt, outcome: body(b)?.outcome };
	let event = panel.log_call(Actor(caller.user_id), &brand, &lead, call, Timestamp::now()).await?;
	Ok(created(json!({ "event_id": event.raw().to_string() })))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PaymentBody {
	billed: i64,
	commission: i64,
	currency: String,
}

async fn payment(
	State(panel): State<Panel>,
	Extension(caller): Extension<Caller>,
	Path((brand, lead)): Path<(String, String)>,
	headers: HeaderMap,
	b: Result<Json<PaymentBody>, JsonRejection>,
) -> ApiResult<Response> {
	allow(caller.role.edits_leads())?;
	let key = idempotency_key(&headers)?;
	let (brand, lead) = ids(&brand, &lead)?;
	let b = body(b)?;
	let p = Payment {
		billed: amount("billed", b.billed)?,
		commission: amount("commission", b.commission)?,
		currency: currency(&b.currency)?,
	};
	let done = panel.record_payment_once(Actor(caller.user_id), &brand, &lead, p, Timestamp::now(), key.as_deref()).await?;
	Ok(recorded(done.replayed, json!({ "event_id": done.value.raw().to_string() })))
}

// ── funnel ──────────────────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FunnelQuery {
	from: Option<String>,
	to: Option<String>,
	brand: Option<String>,
}

/// A share as the front end may show it: `percent` only when `of` is large enough, and
/// `small_sample` saying so otherwise, so "12 of 17" is all that can be drawn.
#[derive(Serialize)]
struct ShareDto {
	n: u64,
	of: u64,
	percent: Option<u64>,
	small_sample: bool,
}

impl From<Share> for ShareDto {
	fn from(s: Share) -> Self {
		Self {
			n: s.n,
			of: s.of,
			percent: s.percent,
			small_sample: s.small_sample(),
		}
	}
}

#[derive(Serialize)]
struct StepDto {
	stage: &'static str,
	reached: u64,
	of_previous: Option<ShareDto>,
	of_leads: ShareDto,
}

async fn funnel(State(panel): State<Panel>, q: Result<Query<FunnelQuery>, axum::extract::rejection::QueryRejection>) -> ApiResult<Json<Value>> {
	let Query(q) = q.map_err(|e| ApiError::BadRequest(e.body_text()))?;
	let day = |raw: &str, what: &str| raw.parse::<Date>().map_err(|_| ApiError::BadRequest(format!("{what} is not a date like 2026-09-30")));
	let today = Timestamp::now().to_zoned(TimeZone::UTC).date();
	let to = q.to.as_deref().map(|d| day(d, "to")).transpose()?.unwrap_or(today);
	let from = match q.from.as_deref() {
		Some(d) => day(d, "from")?,
		None => to.checked_sub(SignedDuration::from_hours(24 * 29)).map_err(|e| ApiError::Internal(e.into()))?,
	};
	if from > to {
		return Err(ApiError::BadRequest("from is after to".into()));
	}
	if (to - from).get_days() >= MAX_FUNNEL_DAYS {
		return Err(ApiError::BadRequest(format!("at most {MAX_FUNNEL_DAYS} days at once")));
	}
	let brand = q.brand.as_deref().map(BrandId::parse).transpose()?;
	let totals = panel.funnel(from, to, brand.as_ref()).await?;
	let stages: Vec<StepDto> = totals
		.steps()
		.into_iter()
		.map(|s| StepDto {
			stage: s.stage.as_str(),
			reached: s.reached,
			of_previous: s.of_previous.map(ShareDto::from),
			of_leads: s.of_leads.into(),
		})
		.collect();
	Ok(Json(json!({
		"from": from.to_string(),
		"to": to.to_string(),
		"brand": brand.as_ref().map(BrandId::as_str),
		"min_sample": MIN_SAMPLE,
		"stages": stages,
		"lost": ShareDto::from(Share::new(totals.lost, totals.leads)),
		"manual": ShareDto::from(Share::new(totals.manual, totals.leads)),
	})))
}

// ── sources (admin) ─────────────────────────────────────────────────────────────────────

#[derive(Serialize)]
struct SourceDto {
	key_id: String,
	kind: &'static str,
	brands: Vec<String>,
	created_at: String,
	revoked_at: Option<String>,
}

async fn sources(State(panel): State<Panel>, Extension(caller): Extension<Caller>) -> ApiResult<Json<Value>> {
	allow(caller.role.manages_sources())?;
	let sources: Vec<SourceDto> = panel
		.store()
		.sources()
		.await?
		.into_iter()
		.map(|s| SourceDto {
			key_id: s.grant.key_id,
			kind: s.grant.kind.as_str(),
			brands: s.grant.brands.iter().map(|b| b.as_str().to_owned()).collect(),
			created_at: s.created_at.to_rfc3339(),
			revoked_at: s.revoked_at.map(|t| t.to_rfc3339()),
		})
		.collect();
	Ok(Json(json!({ "sources": sources })))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AddSource {
	key_id: String,
	kind: String,
	brands: Vec<String>,
}

async fn add_source(State(panel): State<Panel>, Extension(caller): Extension<Caller>, b: Result<Json<AddSource>, JsonRejection>) -> ApiResult<Response> {
	allow(caller.role.manages_sources())?;
	let b = body(b)?;
	let kind: SourceKind = b.kind.parse()?;
	// The panel's own events carry no key (`key_id` NULL): a key of kind panel would only let
	// something outside pass its writes off as typed in by hand.
	if kind == SourceKind::Panel {
		return Err(ApiError::BadRequest("a key of kind panel is not issued: the panel writes without one".into()));
	}
	let brands = b.brands.iter().map(|b| BrandId::parse(b)).collect::<Result<BTreeSet<_>, _>>()?;
	if !panel_core::ids::is_slug(&b.key_id) {
		return Err(ApiError::BadRequest("key_id is a lowercase slug of 1–64 of [a-z0-9_-]".into()));
	}
	if brands.is_empty() {
		return Err(ApiError::BadRequest("a source writes for at least one brand".into()));
	}
	let Some(added) = panel.add_source(&b.key_id, kind, brands).await? else {
		return Err(ApiError::Conflict(format!("a source {} exists already", b.key_id)));
	};
	tracing::info!(user_id = %caller.user_id, key_id = added.key_id, kind = %kind, "source added from the panel");
	// Shown once: the answer is the only place the secret ever is.
	let mut res = created(json!({ "key_id": added.key_id, "secret": added.secret.as_str() }));
	res.headers_mut().insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
	Ok(res)
}

async fn revoke_source(State(panel): State<Panel>, Extension(caller): Extension<Caller>, Path(key_id): Path<String>) -> ApiResult<StatusCode> {
	allow(caller.role.manages_sources())?;
	if !panel.store().revoke_source(&key_id).await? {
		return Err(ApiError::NotFound);
	}
	tracing::info!(user_id = %caller.user_id, key_id, "source revoked from the panel");
	Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
	use panel_core::role::Role;

	use super::*;

	#[test]
	fn cursors_round_trip() {
		let c = ("2026-09-30T10:00:00Z".parse().unwrap(), "aquafix".to_owned(), "L-1".to_owned());
		assert_eq!(cursor_decode(&cursor_encode(&c)).unwrap(), c);
		assert!(cursor_decode("nope!").is_err());
		assert!(cursor_decode(&URL_SAFE_NO_PAD.encode("x|y")).is_err());
	}

	#[test]
	fn operators_do_not_manage_sources() {
		assert!(allow(Role::Operator.manages_sources()).is_err());
		assert!(allow(Role::Admin.manages_sources()).is_ok());
	}
}
