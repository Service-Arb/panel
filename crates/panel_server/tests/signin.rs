//! Signing in and the operator API end to end: the real router and a real Postgres, with a
//! fake concierge speaking the vendored gRPC contract on a local port.

use std::{
	collections::HashMap,
	sync::{Arc, Mutex},
};

use axum::{
	Router,
	body::{Body, to_bytes},
	http::{HeaderMap, Method, Request, StatusCode, header},
};
use jiff::{SignedDuration, Timestamp};
use panel::{
	session::pkce_challenge,
	testing::{TestDb, panel},
};
use panel_contracts::concierge::v1::{
	self as pb,
	auth_service_server::{AuthService, AuthServiceServer},
	user_directory_server::{UserDirectory, UserDirectoryServer},
};
use panel_server::{
	concierge::{CLIENT_ID, Concierge},
	http,
	signin::{SignIn, SignInConfig},
};
use serde_json::{Value, json};
use tonic::{Code, Request as GrpcRequest, Response as GrpcResponse, Status, transport::server::TcpIncoming};
use tower::ServiceExt;

const SECRET: &str = "the-panel-client-secret";
const PANEL: &str = "http://panel.test";
const CODE: &str = "code-1";

// ── the fake concierge ───────────────────────────────────────────────────────────────────

#[derive(Default)]
struct Fake {
	/// The challenge the browser was sent to concierge with, as the test read it off the
	/// redirect: what concierge would have stored with the code.
	challenge: Option<String>,
	/// Who the next code is issued to, and how long its access token lives.
	next_user: Option<(String, SignedDuration)>,
	exchanges: u32,
	refreshes: u32,
	/// access token → the profile `GetMe` answers, or the refusal.
	profiles: HashMap<String, Result<pb::UserProfile, Code>>,
	/// refresh token → user id.
	refresh_tokens: HashMap<String, String>,
	/// How long `ExchangeCode` and `GetMe` take to answer.
	exchange_delay: std::time::Duration,
	me_delay: std::time::Duration,
	me_calls: u32,
	/// The next `GetMe` fails with this, once.
	me_fails_once: Option<Code>,
}

#[derive(Clone, Default)]
struct FakeConcierge(Arc<Mutex<Fake>>);

impl FakeConcierge {
	fn with<R>(&self, f: impl FnOnce(&mut Fake) -> R) -> R {
		f(&mut self.0.lock().unwrap())
	}
}

fn tokens(fake: &mut Fake, user_id: &str, access_ttl: SignedDuration) -> pb::ClientTokenResponse {
	let now = Timestamp::now();
	let n = fake.exchanges + fake.refreshes;
	let access = format!("access-{user_id}-{n}");
	let refresh = format!("refresh-{user_id}-{n}");
	let profile = fake.profiles.values().find_map(|p| p.as_ref().ok().filter(|p| p.user_id == user_id).cloned());
	if let Some(profile) = profile {
		fake.profiles.insert(access.clone(), Ok(profile));
	}
	fake.refresh_tokens.insert(refresh.clone(), user_id.to_owned());
	pb::ClientTokenResponse {
		access_token: access,
		access_expires_at: (now + access_ttl).as_second(),
		refresh_token: refresh,
		refresh_expires_at: (now + SignedDuration::from_hours(24 * 30)).as_second(),
		user_id: user_id.to_owned(),
	}
}

// The panel calls only the RPCs implemented here; the rest exist because the trait does.
#[tonic::async_trait]
impl AuthService for FakeConcierge {
	async fn exchange_code(&self, req: GrpcRequest<pb::ExchangeCodeRequest>) -> Result<GrpcResponse<pb::ClientTokenResponse>, Status> {
		let r = req.into_inner();
		tokio::time::sleep(self.with(|f| f.exchange_delay)).await;
		self.with(|f| {
			f.exchanges += 1;
			if r.client_id != CLIENT_ID || r.client_secret != SECRET {
				return Err(Status::unauthenticated("client"));
			}
			let pkce_ok = f.challenge.as_deref() == Some(pkce_challenge(&r.code_verifier).as_str());
			if r.code != CODE || r.redirect_uri != format!("{PANEL}/auth/callback") || !pkce_ok {
				return Err(Status::unauthenticated("code"));
			}
			let (user, ttl) = f.next_user.clone().ok_or_else(|| Status::permission_denied("policy"))?;
			Ok(GrpcResponse::new(tokens(f, &user, ttl)))
		})
	}

	async fn refresh_client_token(&self, req: GrpcRequest<pb::RefreshClientTokenRequest>) -> Result<GrpcResponse<pb::ClientTokenResponse>, Status> {
		let r = req.into_inner();
		self.with(|f| {
			f.refreshes += 1;
			if r.client_secret != SECRET {
				return Err(Status::unauthenticated("client"));
			}
			let user = f.refresh_tokens.remove(&r.refresh_token).ok_or_else(|| Status::unauthenticated("refresh"))?;
			Ok(GrpcResponse::new(tokens(f, &user, SignedDuration::from_mins(15))))
		})
	}

	async fn exchange(&self, _: GrpcRequest<pb::ExchangeRequest>) -> Result<GrpcResponse<pb::TokenResponse>, Status> {
		Err(Status::unimplemented("exchange"))
	}

	async fn refresh(&self, _: GrpcRequest<pb::RefreshRequest>) -> Result<GrpcResponse<pb::TokenResponse>, Status> {
		Err(Status::unimplemented("refresh"))
	}

	async fn logout(&self, _: GrpcRequest<pb::LogoutRequest>) -> Result<GrpcResponse<pb::LogoutResponse>, Status> {
		Err(Status::unimplemented("logout"))
	}

	async fn list_sessions(&self, _: GrpcRequest<pb::ListSessionsRequest>) -> Result<GrpcResponse<pb::ListSessionsResponse>, Status> {
		Err(Status::unimplemented("list_sessions"))
	}

	async fn revoke_session(&self, _: GrpcRequest<pb::RevokeSessionRequest>) -> Result<GrpcResponse<pb::RevokeSessionResponse>, Status> {
		Err(Status::unimplemented("revoke_session"))
	}

	async fn jwks(&self, _: GrpcRequest<pb::JwksRequest>) -> Result<GrpcResponse<pb::JwksResponse>, Status> {
		Err(Status::unimplemented("jwks"))
	}
}

#[tonic::async_trait]
impl UserDirectory for FakeConcierge {
	async fn get_me(&self, req: GrpcRequest<pb::GetMeRequest>) -> Result<GrpcResponse<pb::UserProfile>, Status> {
		let bearer = req
			.metadata()
			.get("authorization")
			.and_then(|v| v.to_str().ok())
			.and_then(|v| v.strip_prefix("Bearer "))
			.map(str::to_owned);
		tokio::time::sleep(self.with(|f| f.me_delay)).await;
		if let Some(code) = self.with(|f| {
			f.me_calls += 1;
			f.me_fails_once.take()
		}) {
			return Err(Status::new(code, "once"));
		}
		let answer = self.with(|f| bearer.and_then(|b| f.profiles.get(&b).cloned()));
		match answer {
			Some(Ok(p)) => Ok(GrpcResponse::new(p)),
			Some(Err(code)) => Err(Status::new(code, "refused")),
			None => Err(Status::unauthenticated("token")),
		}
	}

	async fn update_profile(&self, _: GrpcRequest<pb::UpdateProfileRequest>) -> Result<GrpcResponse<pb::UserProfile>, Status> {
		Err(Status::unimplemented("update_profile"))
	}

	async fn revoke_tokens(&self, _: GrpcRequest<pb::RevokeTokensRequest>) -> Result<GrpcResponse<pb::RevokeTokensResponse>, Status> {
		Err(Status::unimplemented("revoke_tokens"))
	}

	async fn disable_user(&self, _: GrpcRequest<pb::DisableUserRequest>) -> Result<GrpcResponse<pb::DisableUserResponse>, Status> {
		Err(Status::unimplemented("disable_user"))
	}

	async fn hold_user(&self, _: GrpcRequest<pb::HoldUserRequest>) -> Result<GrpcResponse<pb::HoldUserResponse>, Status> {
		Err(Status::unimplemented("hold_user"))
	}

	async fn reinstate_user(&self, _: GrpcRequest<pb::ReinstateUserRequest>) -> Result<GrpcResponse<pb::ReinstateUserResponse>, Status> {
		Err(Status::unimplemented("reinstate_user"))
	}

	async fn set_kyc_level(&self, _: GrpcRequest<pb::SetKycLevelRequest>) -> Result<GrpcResponse<pb::SetKycLevelResponse>, Status> {
		Err(Status::unimplemented("set_kyc_level"))
	}

	async fn list_users(&self, _: GrpcRequest<pb::ListUsersRequest>) -> Result<GrpcResponse<pb::ListUsersResponse>, Status> {
		Err(Status::unimplemented("list_users"))
	}

	async fn get_user(&self, _: GrpcRequest<pb::GetUserRequest>) -> Result<GrpcResponse<pb::UserProfile>, Status> {
		Err(Status::unimplemented("get_user"))
	}

	async fn set_role(&self, _: GrpcRequest<pb::SetRoleRequest>) -> Result<GrpcResponse<pb::SetRoleResponse>, Status> {
		Err(Status::unimplemented("set_role"))
	}

	async fn grant_scope(&self, _: GrpcRequest<pb::GrantScopeRequest>) -> Result<GrpcResponse<pb::GrantScopeResponse>, Status> {
		Err(Status::unimplemented("grant_scope"))
	}

	async fn revoke_scope(&self, _: GrpcRequest<pb::RevokeScopeRequest>) -> Result<GrpcResponse<pb::RevokeScopeResponse>, Status> {
		Err(Status::unimplemented("revoke_scope"))
	}

	async fn list_scoped_grants(&self, _: GrpcRequest<pb::ListScopedGrantsRequest>) -> Result<GrpcResponse<pb::ListScopedGrantsResponse>, Status> {
		Err(Status::unimplemented("list_scoped_grants"))
	}
}

/// Serves the fake on a free port for the life of the test; its address.
async fn serve_fake(fake: FakeConcierge) -> String {
	let incoming = TcpIncoming::bind("127.0.0.1:0".parse().unwrap()).unwrap();
	let addr = incoming.local_addr().unwrap();
	let server = tonic::transport::Server::builder()
		.add_service(AuthServiceServer::new(fake.clone()))
		.add_service(UserDirectoryServer::new(fake))
		.serve_with_incoming(incoming);
	// Dropped with the test's runtime; a failure shows up as UNAVAILABLE in the assertions.
	let _server = tokio::spawn(server);
	format!("http://{addr}")
}

fn profile(user_id: &str, global: &str, grant: Option<&str>) -> pb::UserProfile {
	pb::UserProfile {
		user_id: user_id.to_owned(),
		email: format!("{global}@example.com"),
		preferred_name: "Ann".to_owned(),
		role: global.to_owned(),
		scopes: grant
			.map(|role| pb::ScopedGrant {
				user_id: user_id.to_owned(),
				scope: "allocation:service_arb".to_owned(),
				role: role.to_owned(),
				..Default::default()
			})
			.into_iter()
			.collect(),
		..Default::default()
	}
}

// ── a browser ────────────────────────────────────────────────────────────────────────────

#[derive(Default)]
struct Browser {
	jar: HashMap<String, String>,
}

struct Answer {
	status: StatusCode,
	headers: HeaderMap,
	body: Value,
}

impl Browser {
	async fn send(&mut self, app: &Router, method: Method, uri: &str, body: Option<Value>, csrf: bool) -> Answer {
		let mut req = Request::builder().method(method).uri(uri);
		let cookie: Vec<String> = self.jar.iter().map(|(k, v)| format!("{k}={v}")).collect();
		if !cookie.is_empty() {
			req = req.header(header::COOKIE, cookie.join("; "));
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
		let headers = res.headers().clone();
		let bytes = to_bytes(res.into_body(), usize::MAX).await.unwrap();
		Answer {
			status,
			headers,
			body: serde_json::from_slice(&bytes).unwrap_or(Value::Null),
		}
	}

	async fn get(&mut self, app: &Router, uri: &str) -> Answer {
		self.send(app, Method::GET, uri, None, false).await
	}

	async fn post(&mut self, app: &Router, uri: &str, body: Value) -> Answer {
		self.send(app, Method::POST, uri, Some(body), true).await
	}

	/// `/auth/login` → (concierge) → `/auth/callback`; the callback's answer.
	async fn sign_in(&mut self, app: &Router, fake: &FakeConcierge, state_override: Option<&str>) -> Answer {
		let login = self.get(app, "/auth/login").await;
		assert_eq!(login.status, StatusCode::FOUND);
		let location = login.headers[header::LOCATION].to_str().unwrap().to_owned();
		assert!(location.starts_with("http://concierge.test/api/auth/authorize?client_id=sa&"), "{location}");
		assert!(location.contains("redirect_uri=http%3A%2F%2Fpanel.test%2Fauth%2Fcallback"), "{location}");
		let param = |name: &str| location.split(['?', '&']).find_map(|p| p.strip_prefix(&format!("{name}="))).unwrap().to_owned();
		fake.with(|f| f.challenge = Some(param("code_challenge")));
		let state = state_override.map_or_else(|| param("state"), str::to_owned);
		self.get(app, &format!("/auth/callback?code={CODE}&state={state}")).await
	}
}

async fn setup(db: &TestDb) -> (Router, FakeConcierge, panel::Panel) {
	setup_with(db, http::Limits::default()).await
}

async fn setup_with(db: &TestDb, limits: http::Limits) -> (Router, FakeConcierge, panel::Panel) {
	let fake = FakeConcierge::default();
	let addr = serve_fake(fake.clone()).await;
	let panel = panel(db).await;
	let concierge = Concierge::new(&addr, SECRET).unwrap();
	let app = http::app_with(
		SignIn::new(
			panel.clone(),
			concierge,
			SignInConfig {
				panel_origin: format!("{PANEL}/"),
				concierge_origin: "http://concierge.test".to_owned(),
			},
		),
		limits,
	);
	(app, fake, panel)
}

const OPERATOR: &str = "0190a7c4-0000-7000-8000-000000000001";
const OUTSIDER: &str = "0190a7c4-0000-7000-8000-000000000002";
const ADMIN: &str = "0190a7c4-0000-7000-8000-000000000003";

// ── the tests ────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn an_operator_signs_in_works_leads_and_signs_out() {
	let Some(db) = TestDb::create().await else { return };
	let (app, fake, _) = setup(&db).await;
	fake.with(|f| {
		f.profiles.insert("seed-operator".into(), Ok(profile(OPERATOR, "investor", Some("operator"))));
		f.next_user = Some((OPERATOR.into(), SignedDuration::from_mins(15)));
	});
	let mut b = Browser::default();

	assert_eq!(b.get(&app, "/api/v1/me").await.status, StatusCode::UNAUTHORIZED, "no session yet");
	let cb = b.sign_in(&app, &fake, None).await;
	assert_eq!(cb.status, StatusCode::SEE_OTHER);
	assert_eq!(cb.headers[header::LOCATION], "/");
	assert_eq!(cb.headers[header::CACHE_CONTROL], "no-store");
	assert!(b.jar.contains_key("sa_session") && b.jar.contains_key("sa_csrf"));
	assert!(!b.jar.contains_key("sa_prelogin"), "the pre-login is single-use");

	let me = b.get(&app, "/api/v1/me").await;
	assert_eq!(me.status, StatusCode::OK, "{}", me.body);
	assert_eq!(
		me.body,
		json!({"user_id": OPERATOR, "role": "operator", "email": "investor@example.com", "preferred_name": "Ann"})
	);
	assert_eq!(me.headers[header::CACHE_CONTROL], "no-store");

	let new_lead = json!({"brand": "aquafix", "location": "paris-11", "need": "a leaking tap", "phone": "+33 6 00 00 00 00"});
	let no_csrf = b.send(&app, Method::POST, "/api/v1/leads", Some(new_lead.clone()), false).await;
	assert_eq!(no_csrf.status, StatusCode::FORBIDDEN, "a write without the CSRF header");
	let created = b.post(&app, "/api/v1/leads", new_lead).await;
	assert_eq!(created.status, StatusCode::CREATED, "{}", created.body);
	let lead_id = created.body["lead_id"].as_str().unwrap().to_owned();
	assert!(lead_id.starts_with("p-"), "{lead_id}");

	let list = b.get(&app, "/api/v1/leads?stage=created").await;
	assert_eq!(list.status, StatusCode::OK, "{}", list.body);
	let row = &list.body["leads"][0];
	assert_eq!(row["lead_id"], lead_id.as_str());
	assert_eq!(row["manual"], true);
	assert_eq!(row["sla"]["overdue"], false);
	assert_eq!(row["pii"]["phone"], "+33 6 00 00 00 00", "an operator sees PII: {row}");

	let attempt = b.post(&app, &format!("/api/v1/leads/aquafix/{lead_id}/calls/attempt"), json!(null)).await;
	assert_eq!(attempt.status, StatusCode::CREATED, "{}", attempt.body);
	let attempt_id = attempt.body["attempt_id"].as_str().unwrap();
	let outcome = b
		.post(&app, &format!("/api/v1/leads/aquafix/{lead_id}/calls/{attempt_id}/outcome"), json!({"outcome": "answered"}))
		.await;
	assert_eq!(outcome.status, StatusCode::CREATED, "{}", outcome.body);
	let contacted = b
		.post(&app, &format!("/api/v1/leads/aquafix/{lead_id}/stage"), json!({"stage": "contacted", "channel": "phone"}))
		.await;
	assert_eq!(contacted.status, StatusCode::CREATED, "{}", contacted.body);
	let card = b.get(&app, &format!("/api/v1/leads/aquafix/{lead_id}")).await;
	assert_eq!(card.status, StatusCode::OK, "{}", card.body);
	assert_eq!(card.body["lead"]["stage"], "contacted", "{}", card.body);
	let types: Vec<&str> = card.body["events"].as_array().unwrap().iter().map(|e| e["type"].as_str().unwrap()).collect();
	assert_eq!(types, ["lead.created", "call.attempted", "call.logged", "lead.contacted"]);

	let bad = b.post(&app, &format!("/api/v1/leads/aquafix/{lead_id}/stage"), json!({"stage": "teleported"})).await;
	assert_eq!(bad.status, StatusCode::BAD_REQUEST);
	assert!(bad.body["error"].is_string());
	assert_eq!(b.get(&app, "/api/v1/leads/aquafix/p-nope").await.status, StatusCode::NOT_FOUND);
	assert_eq!(b.get(&app, "/api/v1/sources").await.status, StatusCode::FORBIDDEN, "sources are an admin's");
	let funnel = b.get(&app, "/api/v1/funnel").await;
	assert_eq!(funnel.status, StatusCode::OK, "{}", funnel.body);
	assert_eq!(funnel.body["stages"][0]["reached"], 1);
	assert_eq!(funnel.body["stages"][0]["of_leads"]["percent"], Value::Null, "1 of 1 is too few for a percentage");

	let no_csrf = b.send(&app, Method::POST, "/auth/logout", None, false).await;
	assert_eq!(no_csrf.status, StatusCode::FORBIDDEN);
	let session = b.jar["sa_session"].clone();
	assert_eq!(b.send(&app, Method::POST, "/auth/logout", None, true).await.status, StatusCode::NO_CONTENT);
	assert!(b.jar.is_empty(), "{:?}", b.jar);
	b.jar.insert("sa_session".into(), session);
	assert_eq!(b.get(&app, "/api/v1/me").await.status, StatusCode::UNAUTHORIZED, "the session is gone server-side");
	assert_eq!(fake.with(|f| f.exchanges), 1);
}

#[tokio::test]
async fn a_forged_callback_never_presents_the_code() {
	let Some(db) = TestDb::create().await else { return };
	let (app, fake, _) = setup(&db).await;
	fake.with(|f| f.next_user = Some((OPERATOR.into(), SignedDuration::from_mins(15))));

	let mut b = Browser::default();
	let cb = b.sign_in(&app, &fake, Some("0000")).await;
	assert_eq!(cb.status, StatusCode::BAD_REQUEST);
	assert!(!b.jar.contains_key("sa_session"));

	let mut stranger = Browser::default();
	let cb = stranger.get(&app, &format!("/auth/callback?code={CODE}&state=abc")).await;
	assert_eq!(cb.status, StatusCode::BAD_REQUEST, "no pre-login cookie at all");
	assert_eq!(fake.with(|f| f.exchanges), 0);

	let cb = stranger.get(&app, "/auth/callback?error=access_denied").await;
	assert_eq!(cb.status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn only_the_scope_lets_in_and_a_refusal_ends_the_session() {
	let Some(db) = TestDb::create().await else { return };
	let (app, fake, _) = setup(&db).await;

	// A plain investor: concierge would not issue a code, but if it did the panel still
	// refuses.
	fake.with(|f| {
		f.profiles.insert("seed-outsider".into(), Ok(profile(OUTSIDER, "investor", None)));
		f.next_user = Some((OUTSIDER.into(), SignedDuration::from_mins(15)));
	});
	let mut outsider = Browser::default();
	assert_eq!(outsider.sign_in(&app, &fake, None).await.status, StatusCode::SEE_OTHER);
	let me = outsider.get(&app, "/api/v1/me").await;
	assert_eq!(me.status, StatusCode::FORBIDDEN, "{}", me.body);

	// A global admin without a grant is an admin here.
	fake.with(|f| {
		f.profiles.insert("seed-admin".into(), Ok(profile(ADMIN, "admin", None)));
		f.next_user = Some((ADMIN.into(), SignedDuration::from_mins(15)));
	});
	let mut admin = Browser::default();
	admin.sign_in(&app, &fake, None).await;
	assert_eq!(admin.get(&app, "/api/v1/me").await.body["role"], "admin");
	let added = admin.post(&app, "/api/v1/sources", json!({"key_id": "vifnet-site", "kind": "site", "brands": ["vifnet"]})).await;
	assert_eq!(added.status, StatusCode::CREATED, "{}", added.body);
	assert_eq!(added.body["secret"].as_str().unwrap().len(), 64);
	let listed = admin.get(&app, "/api/v1/sources").await;
	assert_eq!(listed.body["sources"][0]["key_id"], "vifnet-site");
	assert!(listed.body["sources"][0].get("secret").is_none(), "a secret is shown once");
	let again = admin.post(&app, "/api/v1/sources", json!({"key_id": "vifnet-site", "kind": "site", "brands": ["vifnet"]})).await;
	assert_eq!(again.status, StatusCode::CONFLICT);
	assert_eq!(admin.send(&app, Method::DELETE, "/api/v1/sources/vifnet-site", None, true).await.status, StatusCode::NO_CONTENT);

	// concierge revokes the operator (signed out of evinvest.ltd): GetMe refuses, and the
	// session is closed here too.
	fake.with(|f| {
		f.profiles.insert("seed-operator".into(), Ok(profile(OPERATOR, "investor", Some("operator"))));
		f.next_user = Some((OPERATOR.into(), SignedDuration::from_mins(15)));
	});
	let mut op = Browser::default();
	op.sign_in(&app, &fake, None).await;
	fake.with(|f| {
		for p in f.profiles.values_mut() {
			if p.as_ref().is_ok_and(|p| p.user_id == OPERATOR) {
				*p = Err(Code::Unauthenticated);
			}
		}
	});
	let me = op.get(&app, "/api/v1/me").await;
	assert_eq!(me.status, StatusCode::UNAUTHORIZED, "{}", me.body);
	assert!(!op.jar.contains_key("sa_session"), "the cookie is cleared");
}

#[tokio::test]
async fn a_stale_access_token_is_rotated_once() {
	let Some(db) = TestDb::create().await else { return };
	let (app, fake, _) = setup(&db).await;
	fake.with(|f| {
		f.profiles.insert("seed-operator".into(), Ok(profile(OPERATOR, "investor", Some("operator"))));
		// Inside the refresh margin from the start.
		f.next_user = Some((OPERATOR.into(), SignedDuration::from_secs(5)));
	});
	let mut b = Browser::default();
	b.sign_in(&app, &fake, None).await;
	let me = b.get(&app, "/api/v1/me").await;
	assert_eq!(me.status, StatusCode::OK, "{}", me.body);
	assert_eq!(fake.with(|f| f.refreshes), 1);
	assert_eq!(b.get(&app, "/api/v1/me").await.status, StatusCode::OK);
	assert_eq!(fake.with(|f| f.refreshes), 1, "the rotated token is fresh: no second refresh");
}

#[tokio::test]
async fn concierge_down_is_a_503_not_a_sign_out() {
	let Some(db) = TestDb::create().await else { return };
	let panel = panel(&db).await;
	// Nothing listens there.
	let concierge = Concierge::new("http://127.0.0.1:9", SECRET).unwrap();
	let app = http::app(SignIn::new(
		panel.clone(),
		concierge,
		SignInConfig {
			panel_origin: PANEL.to_owned(),
			concierge_origin: "http://concierge.test".to_owned(),
		},
	));
	let now = Timestamp::now();
	let tokens = panel::session::Tokens {
		access: "a".to_owned().into(),
		refresh: "r".to_owned().into(),
		access_expires_at: now + SignedDuration::from_mins(15),
		refresh_expires_at: now + SignedDuration::from_hours(24),
	};
	let opened = panel.open_session(OPERATOR.parse().unwrap(), &tokens, now).await.unwrap();
	let mut b = Browser::default();
	b.jar.insert("sa_session".into(), opened.cookie.to_string());
	let me = b.get(&app, "/api/v1/me").await;
	assert_eq!(me.status, StatusCode::SERVICE_UNAVAILABLE, "{}", me.body);
	assert!(b.jar.contains_key("sa_session"), "the session survives concierge being down");
}

/// Seeds the fake with a user and makes the next code theirs.
fn user(fake: &FakeConcierge, id: &str, global: &str, grant: Option<&str>) {
	fake.with(|f| {
		f.profiles.insert(format!("seed-{id}"), Ok(profile(id, global, grant)));
		f.next_user = Some((id.into(), SignedDuration::from_mins(15)));
	});
}

// ── security review of #2 ────────────────────────────────────────────────────────────────

#[tokio::test]
async fn a_callback_is_redeemed_once() {
	let Some(db) = TestDb::create().await else { return };
	let (app, fake, _) = setup(&db).await;
	user(&fake, OPERATOR, "investor", Some("operator"));
	let mut b = Browser::default();
	let login = b.get(&app, "/auth/login").await;
	let location = login.headers[header::LOCATION].to_str().unwrap().to_owned();
	let param = |name: &str| location.split(['?', '&']).find_map(|p| p.strip_prefix(&format!("{name}="))).unwrap().to_owned();
	fake.with(|f| f.challenge = Some(param("code_challenge")));
	let prelogin = b.jar["sa_prelogin"].clone();
	let callback = format!("/auth/callback?code={CODE}&state={}", param("state"));
	assert_eq!(b.get(&app, &callback).await.status, StatusCode::SEE_OTHER);

	// The same URL and the same pre-login cookie again, as a replay would have them.
	let mut replay = Browser::default();
	replay.jar.insert("sa_prelogin".into(), prelogin);
	let again = replay.get(&app, &callback).await;
	assert_eq!(again.status, StatusCode::BAD_REQUEST);
	assert!(!replay.jar.contains_key("sa_session"));
	assert_eq!(fake.with(|f| f.exchanges), 1, "the code is presented once");
	let csp = again.headers[header::CONTENT_SECURITY_POLICY].to_str().unwrap();
	assert_eq!(csp, "default-src 'self'; frame-ancestors 'none'; base-uri 'none'; form-action 'self'");
	assert_eq!(again.headers["x-content-type-options"], "nosniff");
}

#[tokio::test]
async fn auth_and_api_shed_and_time_out() {
	let Some(db) = TestDb::create().await else { return };
	let limits = http::Limits {
		auth_concurrent: 1,
		api_timeout: std::time::Duration::from_millis(300),
		..Default::default()
	};
	let (app, fake, _) = setup_with(&db, limits).await;
	user(&fake, OPERATOR, "investor", Some("operator"));
	fake.with(|f| f.exchange_delay = std::time::Duration::from_millis(500));

	// One slow callback holds /auth's only slot: the next /auth request is shed, not queued.
	let mut slow = Browser::default();
	let mut other = Browser::default();
	let (signed_in, shed) = tokio::join!(slow.sign_in(&app, &fake, None), async {
		tokio::time::sleep(std::time::Duration::from_millis(150)).await;
		other.get(&app, "/auth/login").await
	});
	assert_eq!(signed_in.status, StatusCode::SEE_OTHER);
	assert_eq!(shed.status, StatusCode::SERVICE_UNAVAILABLE);
	assert_eq!(shed.headers["x-content-type-options"], "nosniff");

	// GetMe slower than /api/v1's budget: 503, and the session stays.
	fake.with(|f| f.me_delay = std::time::Duration::from_millis(400));
	let me = slow.get(&app, "/api/v1/me").await;
	assert_eq!(me.status, StatusCode::SERVICE_UNAVAILABLE, "{}", me.body);
	fake.with(|f| f.me_delay = std::time::Duration::ZERO);
	assert_eq!(slow.get(&app, "/api/v1/me").await.status, StatusCode::OK);
}

#[tokio::test]
async fn key_changes_ask_concierge_afresh() {
	let Some(db) = TestDb::create().await else { return };
	let (app, fake, _) = setup(&db).await;
	user(&fake, ADMIN, "investor", Some("admin"));
	let mut b = Browser::default();
	b.sign_in(&app, &fake, None).await;
	assert_eq!(b.get(&app, "/api/v1/me").await.body["role"], "admin");

	let panel_key = b.post(&app, "/api/v1/sources", json!({"key_id": "hand", "kind": "panel", "brands": ["aquafix"]})).await;
	assert_eq!(panel_key.status, StatusCode::BAD_REQUEST, "{}", panel_key.body);
	let site = b.post(&app, "/api/v1/sources", json!({"key_id": "aquafix-site", "kind": "site", "brands": ["aquafix"]})).await;
	assert_eq!(site.status, StatusCode::CREATED, "{}", site.body);

	// The grant is taken away at concierge. Reads ride the cache for up to a minute; minting
	// and revoking keys do not.
	fake.with(|f| {
		for p in f.profiles.values_mut().flatten() {
			if p.user_id == ADMIN {
				*p = profile(ADMIN, "investor", None);
			}
		}
	});
	assert_eq!(b.get(&app, "/api/v1/sources").await.status, StatusCode::OK, "cached");
	let minted = b.post(&app, "/api/v1/sources", json!({"key_id": "vifnet-site", "kind": "site", "brands": ["vifnet"]})).await;
	assert_eq!(minted.status, StatusCode::FORBIDDEN, "{}", minted.body);
	let revoked = b.send(&app, Method::DELETE, "/api/v1/sources/aquafix-site", None, true).await;
	assert_eq!(revoked.status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn signing_in_again_and_out_closes_the_old_sessions() {
	let Some(db) = TestDb::create().await else { return };
	let (app, fake, _) = setup(&db).await;
	user(&fake, OPERATOR, "investor", Some("operator"));

	let mut laptop = Browser::default();
	laptop.sign_in(&app, &fake, None).await;
	let first = laptop.jar["sa_session"].clone();
	fake.with(|f| f.next_user = Some((OPERATOR.into(), SignedDuration::from_mins(15))));
	laptop.sign_in(&app, &fake, None).await;
	assert_ne!(laptop.jar["sa_session"], first);
	let mut stale = Browser::default();
	stale.jar.insert("sa_session".into(), first);
	assert_eq!(stale.get(&app, "/api/v1/me").await.status, StatusCode::UNAUTHORIZED, "the replaced session is closed");

	let mut phone = Browser::default();
	fake.with(|f| f.next_user = Some((OPERATOR.into(), SignedDuration::from_mins(15))));
	phone.sign_in(&app, &fake, None).await;
	assert_eq!(phone.get(&app, "/api/v1/me").await.status, StatusCode::OK, "cached now");
	assert_eq!(laptop.send(&app, Method::POST, "/auth/logout", None, true).await.status, StatusCode::NO_CONTENT);
	assert_eq!(phone.get(&app, "/api/v1/me").await.status, StatusCode::UNAUTHORIZED, "signed out everywhere");
}

#[tokio::test]
async fn money_is_bounded_and_writes_are_idempotent() {
	let Some(db) = TestDb::create().await else { return };
	let (app, fake, _) = setup(&db).await;
	user(&fake, OPERATOR, "investor", Some("operator"));
	fake.with(|f| f.me_fails_once = Some(Code::DeadlineExceeded));
	let mut b = Browser::default();
	b.sign_in(&app, &fake, None).await;
	assert_eq!(b.get(&app, "/api/v1/me").await.status, StatusCode::OK, "a GetMe timeout is asked once more");
	assert_eq!(fake.with(|f| f.me_calls), 2);

	let new_lead = json!({"brand": "aquafix", "location": "paris-11", "need": "a leaking tap"});
	let with_key = |b: &mut Browser, uri: String, body: Value, key: &'static str| {
		let app = app.clone();
		let cookie: Vec<String> = b.jar.iter().map(|(k, v)| format!("{k}={v}")).collect();
		let csrf = b.jar["sa_csrf"].clone();
		async move {
			let req = Request::post(uri)
				.header(header::COOKIE, cookie.join("; "))
				.header("x-sa-csrf", csrf)
				.header("idempotency-key", key)
				.header(header::CONTENT_TYPE, "application/json")
				.body(Body::from(body.to_string()))
				.unwrap();
			let res = app.oneshot(req).await.unwrap();
			let status = res.status();
			let body: Value = serde_json::from_slice(&to_bytes(res.into_body(), usize::MAX).await.unwrap()).unwrap_or(Value::Null);
			(status, body)
		}
	};
	let (s1, first) = with_key(&mut b, "/api/v1/leads".into(), new_lead.clone(), "lead-1").await;
	let (s2, again) = with_key(&mut b, "/api/v1/leads".into(), new_lead.clone(), "lead-1").await;
	assert_eq!((s1, s2), (StatusCode::CREATED, StatusCode::OK), "{first} {again}");
	assert_eq!(first, again, "the retry is answered with the first lead");
	let lead = first["lead_id"].as_str().unwrap().to_owned();
	let (_, other) = with_key(&mut b, "/api/v1/leads".into(), new_lead, "lead-2").await;
	assert_ne!(other["lead_id"], lead.as_str(), "another key, another lead");

	b.post(&app, &format!("/api/v1/leads/aquafix/{lead}/stage"), json!({"stage": "won"})).await;
	let pay = |billed: i64, currency: &str| json!({"billed": billed, "commission": 0, "currency": currency});
	let too_much = b.post(&app, &format!("/api/v1/leads/aquafix/{lead}/payments"), pay(10_000_000_001, "EUR")).await;
	assert_eq!(too_much.status, StatusCode::BAD_REQUEST, "{}", too_much.body);
	let yen = b.post(&app, &format!("/api/v1/leads/aquafix/{lead}/payments"), pay(100, "JPY")).await;
	assert_eq!(yen.status, StatusCode::BAD_REQUEST, "{}", yen.body);
	let quote = b
		.post(
			&app,
			&format!("/api/v1/leads/aquafix/{lead}/stage"),
			json!({"stage": "quoted", "amount": -10_000_000_001_i64, "currency": "EUR"}),
		)
		.await;
	assert_eq!(quote.status, StatusCode::BAD_REQUEST, "{}", quote.body);

	let uri = format!("/api/v1/leads/aquafix/{lead}/payments");
	let (p1, paid) = with_key(&mut b, uri.clone(), pay(12_000, "EUR"), "pay-1").await;
	let (p2, paid_again) = with_key(&mut b, uri, pay(12_000, "EUR"), "pay-1").await;
	assert_eq!((p1, p2), (StatusCode::CREATED, StatusCode::OK), "{paid} {paid_again}");
	assert_eq!(paid["event_id"], paid_again["event_id"]);
	let card = b.get(&app, &format!("/api/v1/leads/aquafix/{lead}")).await;
	let payments = card.body["events"].as_array().unwrap().iter().filter(|e| e["type"] == "payment.received").count();
	assert_eq!(payments, 1, "the retry journaled nothing");
	assert_eq!(card.headers["x-content-type-options"], "nosniff");
}
