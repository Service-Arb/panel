//! Signing in through concierge (spec §4), and the gate every `/api/v1` request passes.
//!
//! ```text
//! GET /auth/login      state + PKCE verifier sealed into the pre-login cookie
//!                      → 302 concierge /api/auth/authorize?client_id=sa&…&code_challenge
//! GET /auth/callback   state from the cookie = state in the URL (constant time), else 400
//!                      and the code is never presented; ExchangeCode(code, verifier)
//!                      → server-side session, cookies → 303 /
//! /api/v1/*            CSRF on anything but GET; session (access token rotated when it is
//!                      about to expire); GetMe (≤ 60 s cache) → role, else 401/403/503
//! POST /auth/logout    CSRF; the session is deleted
//! ```
//!
//! The browser never holds a concierge token: only a random session id, whose hash names a
//! row holding the tokens sealed.

use std::{
	collections::HashMap,
	sync::{Arc, Mutex},
	time::{Duration, Instant},
};

use axum::{
	Json,
	extract::{Query, Request, State},
	http::{HeaderMap, HeaderValue, Method, StatusCode, header},
	middleware::Next,
	response::{Html, IntoResponse, Response},
};
use jiff::Timestamp;
use panel::{
	Panel,
	session::{PRELOGIN_TTL, SessionError, SessionKey},
};
use panel_core::role::Role;
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use crate::{
	concierge::{CLIENT_ID, Concierge, ConciergeError, Me},
	cookies::{self, Cookies},
};

/// How long `GetMe` is trusted for a session: a grant revoked at concierge shuts the panel
/// at most this much later (at the next token rotation, concierge refuses outright).
pub const ME_TTL: Duration = Duration::from_secs(60);

/// What signing in needs.
#[derive(Clone, Debug)]
pub struct SignInConfig {
	/// e.g. `https://sa.evinvest.ltd`, no trailing slash.
	pub panel_origin: String,
	/// e.g. `https://evinvest.ltd`.
	pub concierge_origin: String,
}

/// The sign-in's state, shared by the routes and the gate.
#[derive(Clone, Debug)]
pub struct SignIn {
	pub panel: Panel,
	pub concierge: Concierge,
	config: Arc<SignInConfig>,
	cookies: Cookies,
	me: Arc<Mutex<HashMap<SessionKey, (Instant, Me)>>>,
}

impl SignIn {
	pub fn new(panel: Panel, concierge: Concierge, mut config: SignInConfig) -> Self {
		config.panel_origin = config.panel_origin.trim_end_matches('/').to_owned();
		config.concierge_origin = config.concierge_origin.trim_end_matches('/').to_owned();
		let cookies = Cookies::new(config.panel_origin.starts_with("https://"));
		Self {
			panel,
			concierge,
			config: Arc::new(config),
			cookies,
			me: Arc::default(),
		}
	}

	pub fn cookies(&self) -> &Cookies {
		&self.cookies
	}

	fn redirect_uri(&self) -> String {
		format!("{}/auth/callback", self.config.panel_origin)
	}

	fn cached_me(&self, key: &SessionKey) -> Option<Me> {
		let cache = self.me.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
		cache.get(key).filter(|(at, _)| at.elapsed() < ME_TTL).map(|(_, me)| me.clone())
	}

	fn remember_me(&self, key: SessionKey, me: Me) {
		let mut cache = self.me.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
		// Bounded without a sweeper: stale entries go whenever the map has grown.
		if cache.len() >= 1024 {
			cache.retain(|_, (at, _)| at.elapsed() < ME_TTL);
		}
		cache.insert(key, (Instant::now(), me));
	}

	fn forget_me(&self, key: &SessionKey) {
		self.me.lock().unwrap_or_else(std::sync::PoisonError::into_inner).remove(key);
	}
}

/// The signed-in user of an `/api/v1` request, put in its extensions by [`gate`].
#[derive(Clone, Debug)]
pub struct Caller {
	pub user_id: Uuid,
	pub role: Role,
	pub email: String,
	pub preferred_name: String,
}

fn json_error(status: StatusCode, msg: &str) -> Response {
	(status, Json(json!({ "error": msg }))).into_response()
}

/// A page on this origin with fixed text: nothing from the request is echoed.
fn page(status: StatusCode, title: &str, message: &str) -> Response {
	let body = format!(
		"<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\">\
		 <title>{title}</title></head><body><main><h1>{title}</h1><p>{message}</p><p><a href=\"/auth/login\">Sign in again</a></p></main></body></html>"
	);
	(status, Html(body)).into_response()
}

/// The sign-in's answers carry a code or a token in their URLs: kept out of caches and out
/// of the `Referer` of whatever the page loads next.
fn private(mut res: Response) -> Response {
	let h = res.headers_mut();
	h.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
	h.insert(header::REFERRER_POLICY, HeaderValue::from_static("no-referrer"));
	res
}

/// Percent-encodes a query value: everything but RFC 3986's unreserved characters.
fn encode(value: &str) -> String {
	let mut out = String::with_capacity(value.len());
	for b in value.bytes() {
		if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~') {
			out.push(char::from(b));
		} else {
			out.push_str(&format!("%{b:02X}"));
		}
	}
	out
}

/// `GET /auth/login`.
pub async fn login(State(s): State<SignIn>) -> Response {
	let begun = match s.panel.begin_sign_in(Timestamp::now()) {
		Ok(b) => b,
		Err(e) => {
			crate::report(&e, "starting a sign-in");
			return private(page(StatusCode::INTERNAL_SERVER_ERROR, "Sign-in failed", "Something went wrong on our side. Try again."));
		}
	};
	let location = format!(
		"{}/api/auth/authorize?client_id={CLIENT_ID}&redirect_uri={}&response_type=code&state={}&code_challenge={}&code_challenge_method=S256",
		s.config.concierge_origin,
		encode(&s.redirect_uri()),
		begun.state,
		begun.challenge,
	);
	let Ok(location) = HeaderValue::from_str(&location) else {
		return private(page(StatusCode::INTERNAL_SERVER_ERROR, "Sign-in failed", "The sign-in is misconfigured."));
	};
	let prelogin = s.cookies.set(cookies::PRELOGIN, &begun.cookie, PRELOGIN_TTL.as_secs(), true);
	private((StatusCode::FOUND, [(header::LOCATION, location), (header::SET_COOKIE, prelogin)]).into_response())
}

#[derive(Deserialize)]
pub struct Callback {
	code: Option<String>,
	state: Option<String>,
	error: Option<String>,
}

/// `GET /auth/callback`.
pub async fn callback(State(s): State<SignIn>, headers: HeaderMap, Query(q): Query<Callback>) -> Response {
	// The pre-login is single-use whatever happens next.
	let clear = s.cookies.clear(cookies::PRELOGIN);
	let with_clear = |res: Response| {
		let mut res = private(res);
		res.headers_mut().append(header::SET_COOKIE, clear.clone());
		res
	};
	if let Some(error) = q.error.as_deref() {
		// Fixed pages: concierge's error word picks one, and is not shown.
		return with_clear(match error {
			"access_denied" => page(StatusCode::FORBIDDEN, "No access", "Your account has no access to the Service-Arb panel. Ask its admin for one."),
			"temporarily_unavailable" => page(
				StatusCode::SERVICE_UNAVAILABLE,
				"Sign-in unavailable",
				"Signing in is unavailable right now. Try again in a minute.",
			),
			_ => page(StatusCode::BAD_REQUEST, "Sign-in failed", "The sign-in could not be completed. Try again."),
		});
	}
	let now = Timestamp::now();
	let pre = match (s.cookies.get(&headers, cookies::PRELOGIN), q.state.as_deref()) {
		(Some(cookie), Some(state)) => s.panel.finish_sign_in(cookie, state, now),
		_ => None,
	};
	let (Some(pre), Some(code)) = (pre, q.code.as_deref().filter(|c| !c.is_empty())) else {
		tracing::info!("sign-in callback without a matching pre-login; the code is not presented");
		return with_clear(page(
			StatusCode::BAD_REQUEST,
			"Sign-in failed",
			"This sign-in link is not valid here, or has expired. Start again.",
		));
	};
	let issued = match s.concierge.exchange_code(code, &s.redirect_uri(), &pre.verifier).await {
		Ok(i) => i,
		Err(ConciergeError::Refused(tonic::Code::PermissionDenied)) =>
			return with_clear(page(
				StatusCode::FORBIDDEN,
				"No access",
				"Your account has no access to the Service-Arb panel. Ask its admin for one.",
			)),
		Err(ConciergeError::Refused(_)) => return with_clear(page(StatusCode::BAD_REQUEST, "Sign-in failed", "This sign-in has expired or was already used. Start again.")),
		Err(ConciergeError::Unavailable(why)) => {
			tracing::warn!(why, "concierge unavailable at the sign-in callback");
			return with_clear(page(
				StatusCode::SERVICE_UNAVAILABLE,
				"Sign-in unavailable",
				"Signing in is unavailable right now. Try again in a minute.",
			));
		}
		Err(e @ ConciergeError::Failed(_)) => {
			crate::report(&eyre::eyre!(e), "redeeming a sign-in code");
			return with_clear(page(StatusCode::BAD_GATEWAY, "Sign-in failed", "Something went wrong. Try again."));
		}
	};
	let opened = match s.panel.open_session(issued.user_id, &issued.tokens, now).await {
		Ok(o) => o,
		Err(e) => {
			crate::report(&e, "opening a session");
			return with_clear(page(StatusCode::INTERNAL_SERVER_ERROR, "Sign-in failed", "Something went wrong on our side. Try again."));
		}
	};
	let csrf = match panel::session::random_token() {
		Ok(t) => t,
		Err(e) => {
			crate::report(&e, "making a CSRF token");
			return with_clear(page(StatusCode::INTERNAL_SERVER_ERROR, "Sign-in failed", "Something went wrong on our side. Try again."));
		}
	};
	let max_age = opened.expires_at.duration_since(now).as_secs();
	tracing::info!(user_id = %issued.user_id, "signed in");
	let mut res = with_clear((StatusCode::SEE_OTHER, [(header::LOCATION, HeaderValue::from_static("/"))]).into_response());
	let h = res.headers_mut();
	h.append(header::SET_COOKIE, s.cookies.set(cookies::SESSION, &opened.cookie, max_age, true));
	h.append(header::SET_COOKIE, s.cookies.set(cookies::CSRF, &csrf, max_age, false));
	res
}

/// `POST /auth/logout`: CSRF-checked; the session is deleted and the cookies cleared.
pub async fn logout(State(s): State<SignIn>, headers: HeaderMap) -> Response {
	if !s.cookies.csrf_ok(&headers) {
		return json_error(StatusCode::FORBIDDEN, "csrf");
	}
	if let Some(key) = s.cookies.get(&headers, cookies::SESSION).and_then(SessionKey::of_cookie) {
		s.forget_me(&key);
		if let Err(e) = s.panel.close_session(&key).await {
			crate::report(&e, "closing a session");
			return json_error(StatusCode::INTERNAL_SERVER_ERROR, "internal error");
		}
	}
	signed_out(&s, StatusCode::NO_CONTENT.into_response())
}

fn signed_out(s: &SignIn, mut res: Response) -> Response {
	let h = res.headers_mut();
	h.append(header::SET_COOKIE, s.cookies.clear(cookies::SESSION));
	h.append(header::SET_COOKIE, s.cookies.clear(cookies::CSRF));
	res
}

/// The gate of `/api/v1`: CSRF for anything that is not a read, then the session and the
/// caller's role, into the request's extensions as a [`Caller`].
pub async fn gate(State(s): State<SignIn>, mut req: Request, next: Next) -> Response {
	let safe = matches!(*req.method(), Method::GET | Method::HEAD | Method::OPTIONS);
	if !safe && !s.cookies.csrf_ok(req.headers()) {
		return json_error(StatusCode::FORBIDDEN, "csrf");
	}
	let caller = match authenticate(&s, req.headers()).await {
		Ok(c) => c,
		Err(res) => return *res,
	};
	req.extensions_mut().insert(caller);
	let mut res = next.run(req).await;
	// PII and secrets pass through here: nothing is cached.
	res.headers_mut().insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
	res
}

/// The refusal is boxed: a `Response` is large, and the happy path should not carry it.
async fn authenticate(s: &SignIn, headers: &HeaderMap) -> Result<Caller, Box<Response>> {
	let unauthenticated = || Box::new(signed_out(s, json_error(StatusCode::UNAUTHORIZED, "sign in")));
	let unavailable = || Box::new(json_error(StatusCode::SERVICE_UNAVAILABLE, "sign-in is unavailable, try again"));
	let cookie = s.cookies.get(headers, cookies::SESSION).ok_or_else(unauthenticated)?;
	let session = match s.panel.session(cookie, Timestamp::now(), &s.concierge).await {
		Ok(session) => session,
		Err(SessionError::Missing | SessionError::Rejected) => return Err(unauthenticated()),
		Err(SessionError::Unavailable) => return Err(unavailable()),
		Err(SessionError::Internal(e)) => {
			crate::report(&e, "reading a session");
			return Err(Box::new(json_error(StatusCode::INTERNAL_SERVER_ERROR, "internal error")));
		}
	};
	let me = match s.cached_me(&session.key) {
		Some(me) => me,
		None => match s.concierge.me(&session.access).await {
			Ok(me) => {
				s.remember_me(session.key, me.clone());
				me
			}
			// Revoked at concierge (signed out of evinvest.ltd, access taken away): the session
			// is over here too.
			Err(ConciergeError::Refused(code)) => {
				tracing::info!(user_id = %session.user_id, ?code, "GetMe refused; session closed");
				s.forget_me(&session.key);
				if let Err(e) = s.panel.close_session(&session.key).await {
					crate::report(&e, "closing a refused session");
				}
				return Err(unauthenticated());
			}
			Err(ConciergeError::Unavailable(why)) => {
				tracing::warn!(why, "concierge unavailable for GetMe");
				return Err(unavailable());
			}
			Err(e @ ConciergeError::Failed(_)) => {
				crate::report(&eyre::eyre!(e), "GetMe");
				return Err(Box::new(json_error(StatusCode::BAD_GATEWAY, "sign-in failed")));
			}
		},
	};
	if me.user_id != session.user_id {
		crate::report(&eyre::eyre!("GetMe answered another user than the session's"), "GetMe");
		return Err(Box::new(json_error(StatusCode::BAD_GATEWAY, "sign-in failed")));
	}
	let grants = me.scopes.iter().map(|(scope, role)| (scope.as_str(), role.as_str()));
	let Some(role) = Role::admitted(&me.role, grants) else {
		return Err(Box::new(json_error(StatusCode::FORBIDDEN, "no access to the panel")));
	};
	Ok(Caller {
		user_id: me.user_id,
		role,
		email: me.email,
		preferred_name: me.preferred_name,
	})
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn query_values_are_encoded() {
		assert_eq!(encode("https://sa.evinvest.ltd/auth/callback"), "https%3A%2F%2Fsa.evinvest.ltd%2Fauth%2Fcallback");
		assert_eq!(encode("aZ09-._~ &="), "aZ09-._~%20%26%3D");
	}
}
