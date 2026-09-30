//! The Telegram bot end to end on a real Postgres, against a mock Bot API served in-process
//! (axum on a local port — never the network) and a fake concierge: linking, the rules'
//! fan-out, the outbox's pace and retries, and the buttons.
//!
//! Time is virtual: every pass is given its `now`, so the pace is checked to the millisecond
//! without sleeping.

use std::{
	collections::{HashMap, VecDeque},
	sync::{Arc, Mutex},
};

use axum::{
	Json, Router,
	extract::{Path, State},
	http::StatusCode,
	routing::post,
};
use jiff::{SignedDuration, Timestamp};
use panel::{
	Panel,
	operator::{Actor, NewLead, Payment, Pii},
	session::{RefreshError, Refresher, Tokens},
	telegram::{Chat, Directory, DirectoryError, Identity, Notifier, Update},
	testing::{TestDb, panel},
};
use panel_core::{
	ids::{BrandId, LeadId, LocationId},
	lead::Stage,
	notify::Locale,
	role::Role,
};
use panel_server::telegram::BotApi;
use serde_json::{Value, json};
use uuid::Uuid;
use zeroize::Zeroizing;

const TOKEN: &str = "123456:test-bot-token";

fn t0() -> Timestamp {
	"2026-09-30T18:00:00Z".parse().unwrap()
}

fn at(ms: i64) -> Timestamp {
	t0() + SignedDuration::from_millis(ms)
}

// ── the mock Bot API ─────────────────────────────────────────────────────────────────────

#[derive(Default)]
struct Mock {
	/// Every call, in order: `(method, body)`.
	calls: Vec<(String, Value)>,
	/// What the next sendMessage calls answer, in order; then 200.
	send_script: VecDeque<(StatusCode, Value)>,
	next_message_id: i64,
}

#[derive(Clone, Default)]
struct MockBot(Arc<Mutex<Mock>>);

impl MockBot {
	fn with<R>(&self, f: impl FnOnce(&mut Mock) -> R) -> R {
		f(&mut self.0.lock().unwrap())
	}

	fn calls(&self, method: &str) -> Vec<Value> {
		self.with(|m| m.calls.iter().filter(|(k, _)| k == method).map(|(_, b)| b.clone()).collect())
	}

	fn script(&self, status: u16, body: Value) {
		self.with(|m| m.send_script.push_back((StatusCode::from_u16(status).unwrap(), body)));
	}

	fn clear(&self) {
		self.with(|m| m.calls.clear());
	}
}

async fn bot_method(State(mock): State<MockBot>, Path((bot, method)): Path<(String, String)>, Json(body): Json<Value>) -> (StatusCode, Json<Value>) {
	assert_eq!(bot, format!("bot{TOKEN}"), "the token is in the path");
	mock.with(|m| {
		m.calls.push((method.clone(), body.clone()));
		if method == "sendMessage"
			&& let Some((status, answer)) = m.send_script.pop_front()
		{
			return (status, Json(answer));
		}
		m.next_message_id += 1;
		let result = match method.as_str() {
			"sendMessage" => json!({"message_id": m.next_message_id, "chat": {"id": body["chat_id"]}}),
			"getMe" => json!({"id": 1, "is_bot": true, "username": "evinvest_sa_bot"}),
			_ => json!(true),
		};
		(StatusCode::OK, Json(json!({"ok": true, "result": result})))
	})
}

async fn mock_bot() -> (MockBot, String) {
	let mock = MockBot::default();
	let app = Router::new().route("/{bot}/{method}", post(bot_method)).with_state(mock.clone());
	let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
	let base = format!("http://{}", listener.local_addr().unwrap());
	// Lives as long as the test's runtime.
	tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
	(mock, base)
}

// ── the fake concierge ───────────────────────────────────────────────────────────────────

/// access token → who `GetMe` says holds it, or a refusal.
#[derive(Clone, Default)]
struct FakeConcierge(Arc<Mutex<HashMap<String, Result<Identity, ()>>>>);

impl Refresher for FakeConcierge {
	async fn refresh(&self, _: &str) -> Result<Tokens, RefreshError> {
		// The tokens the tests open sessions with never come near expiry.
		Err(RefreshError::Rejected)
	}
}

impl Directory for FakeConcierge {
	async fn me(&self, access: &str) -> Result<Identity, DirectoryError> {
		match self.0.lock().unwrap().get(access).cloned() {
			Some(Ok(id)) => Ok(id),
			Some(Err(())) | None => Err(DirectoryError::Refused),
		}
	}
}

struct Setup {
	_db: TestDb,
	db: sqlx::PgPool,
	panel: Panel,
	mock: MockBot,
	concierge: FakeConcierge,
	n: Notifier<BotApi, FakeConcierge>,
}

async fn setup() -> Option<Setup> {
	let db = TestDb::create().await?;
	let panel = panel(&db).await;
	let (mock, base) = mock_bot().await;
	let concierge = FakeConcierge::default();
	let n = Notifier {
		panel: panel.clone(),
		bot: BotApi::new(&base, TOKEN).unwrap(),
		concierge: concierge.clone(),
		locale: Locale::Ru,
	};
	Some(Setup {
		db: db.pool().await,
		_db: db,
		panel,
		mock,
		concierge,
		n,
	})
}

fn private(id: i64) -> Chat {
	Chat { id, private: true }
}

impl Setup {
	/// A user linked to `chat` at `now`, as the profile and `/start` do it.
	async fn link(&self, user: Uuid, role: Role, chat: i64, now: Timestamp) {
		let token = self.panel.telegram_link_token(user, role, &format!("user-{chat}"), now).await.unwrap();
		self.n
			.handle(
				Update::Start {
					chat: private(chat),
					payload: Some(token.to_string()),
				},
				now,
			)
			.await
			.unwrap();
	}

	/// A live panel session for `user`, whose access token concierge answers `GetMe` for.
	async fn session(&self, user: Uuid, role: Option<Role>, name: &str) {
		let access = format!("access-{user}");
		let far = t0() + SignedDuration::from_hours(24 * 30);
		let tokens = Tokens {
			access: Zeroizing::new(access.clone()),
			access_expires_at: far,
			refresh: Zeroizing::new(format!("refresh-{user}")),
			refresh_expires_at: far,
		};
		self.panel.open_session(user, &tokens, t0()).await.unwrap();
		self.concierge.0.lock().unwrap().insert(
			access,
			Ok(Identity {
				user_id: user,
				role,
				display_name: name.to_owned(),
			}),
		);
	}

	async fn lead(&self, now: Timestamp) -> LeadId {
		let (lead, _) = self
			.panel
			.create_lead(
				Actor(Uuid::now_v7()),
				NewLead {
					brand: brand(),
					location: LocationId::parse("paris-11").unwrap(),
					need: "a leaking tap".into(),
					phone: Some("+33 6 00 00 00 00".into()),
				},
				now,
			)
			.await
			.unwrap();
		lead
	}

	async fn outbox(&self) -> Vec<(i64, String, String, i32)> {
		sqlx::query_as("SELECT chat_id, rule, state, attempts FROM telegram_outbox ORDER BY id")
			.fetch_all(&self.db)
			.await
			.unwrap()
	}

	async fn linked(&self, user: Uuid) -> bool {
		self.panel.telegram_settings(user, Role::Admin).await.unwrap().linked
	}

	async fn events_of(&self, lead: &LeadId, r#type: &str) -> usize {
		let (_, events) = self.panel.lead_card(&brand(), lead, Pii::Withhold, t0()).await.unwrap().unwrap();
		events.iter().filter(|e| e.row.r#type == r#type).count()
	}
}

fn brand() -> BrandId {
	BrandId::parse("aquafix").unwrap()
}

fn texts(calls: &[Value]) -> Vec<String> {
	calls.iter().map(|c| c["text"].as_str().unwrap_or_default().to_owned()).collect()
}

// ── linking ──────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn a_link_token_is_single_use_private_and_expires() {
	let Some(s) = setup().await else { return };
	let (alice, bob) = (Uuid::now_v7(), Uuid::now_v7());

	let token = s.panel.telegram_link_token(alice, Role::Operator, "Alice", t0()).await.unwrap();
	assert!(token.len() >= 43, "256 bits, base64url");
	let raw: Option<(Vec<u8>,)> = sqlx::query_as("SELECT token_hash FROM telegram_link_tokens").fetch_optional(&s.db).await.unwrap();
	assert_ne!(raw.unwrap().0, token.as_bytes(), "only a hash is stored");

	// From a group: ignored whatever it carries, and the token is not spent.
	let group = Update::Start {
		chat: Chat { id: -100, private: false },
		payload: Some(token.to_string()),
	};
	s.n.handle(group, t0()).await.unwrap();
	assert!(!s.linked(alice).await);
	assert!(s.mock.calls("sendMessage").is_empty(), "no answer in a group");

	let start = |chat, token: &str| Update::Start {
		chat: private(chat),
		payload: Some(token.to_owned()),
	};
	s.n.handle(start(7, &token), t0() + SignedDuration::from_mins(9)).await.unwrap();
	assert!(s.linked(alice).await);
	assert!(texts(&s.mock.calls("sendMessage"))[0].starts_with("Telegram подключён"));

	// Spent: a second /start with it links nothing, even from another chat.
	s.mock.clear();
	s.n.handle(start(8, &token), t0() + SignedDuration::from_mins(9)).await.unwrap();
	assert!(texts(&s.mock.calls("sendMessage"))[0].starts_with("Ссылка недействительна"));
	let chat: i64 = sqlx::query_scalar("SELECT chat_id FROM telegram_links WHERE user_id = $1")
		.bind(alice)
		.fetch_one(&s.db)
		.await
		.unwrap();
	assert_eq!(chat, 7);

	// Expired: past ten minutes it opens nothing.
	let late = s.panel.telegram_link_token(bob, Role::Operator, "Bob", t0()).await.unwrap();
	s.n.handle(start(9, &late), t0() + SignedDuration::from_mins(10)).await.unwrap();
	assert!(!s.linked(bob).await);
	let forged = start(9, "A".repeat(43).as_str());
	s.n.handle(forged, t0()).await.unwrap();
	assert!(!s.linked(bob).await);

	// /stop unlinks; the profile's DELETE does too.
	s.n.handle(Update::Stop { chat: private(7) }, t0()).await.unwrap();
	assert!(!s.linked(alice).await);
	s.link(alice, Role::Operator, 7, t0()).await;
	assert!(s.panel.telegram_unlink(alice).await.unwrap());
	assert!(!s.panel.telegram_unlink(alice).await.unwrap());
}

// ── the rules ────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn a_new_lead_goes_to_linked_users_with_access_only() {
	let Some(s) = setup().await else { return };
	let operator = Uuid::now_v7(); // linked, on by default: gets it
	let muted = Uuid::now_v7(); // turned the rule off
	let revoked = Uuid::now_v7(); // concierge said no since
	let stale = Uuid::now_v7(); // not confirmed for over an hour
	let _unlinked = Uuid::now_v7();
	s.link(operator, Role::Operator, 1, t0()).await;
	s.link(muted, Role::Admin, 2, t0()).await;
	s.link(revoked, Role::Operator, 3, t0()).await;
	s.link(stale, Role::Operator, 4, t0() - SignedDuration::from_mins(61)).await;
	s.panel.telegram_set_rules(muted, Role::Admin, &[(panel_core::notify::Rule::NewLead, false)]).await.unwrap();
	s.panel.telegram_access_seen(revoked, None, "Rex", t0()).await.unwrap();
	assert!(
		s.panel
			.telegram_set_rules(operator, Role::Operator, &[(panel_core::notify::Rule::PaymentReceived, true)])
			.await
			.is_err(),
		"payments are the admins'"
	);
	s.mock.clear();

	let lead = s.lead(t0()).await;
	assert_eq!(s.panel.telegram_fan_out(t0(), Locale::Ru).await.unwrap(), 1);
	assert_eq!(s.panel.telegram_fan_out(t0(), Locale::Ru).await.unwrap(), 0, "told once");
	assert_eq!(s.outbox().await, [(1, "new_lead".to_owned(), "pending".to_owned(), 0)]);
	let sealed: Vec<u8> = sqlx::query_scalar("SELECT text_sealed FROM telegram_outbox").fetch_one(&s.db).await.unwrap();
	assert!(!String::from_utf8_lossy(&sealed).contains("+33"), "the phone is sealed in the outbox");

	let done = s.n.deliver(t0()).await.unwrap();
	assert_eq!(done.sent, 1);
	let sent = s.mock.calls("sendMessage");
	assert_eq!(sent.len(), 1);
	assert_eq!(sent[0]["chat_id"], 1);
	assert_eq!(
		sent[0]["text"], "Новая заявка\naquafix · paris-11\nНужно: a leaking tap\nТелефон: +33 6 00 00 00 00",
		"PII for a role that sees it in the panel"
	);
	let labels: Vec<&str> = sent[0]["reply_markup"]["inline_keyboard"]
		.as_array()
		.unwrap()
		.iter()
		.map(|row| row[0]["text"].as_str().unwrap())
		.collect();
	assert_eq!(labels, ["Взял", "Не дозвонился"]);
	let text: Option<Vec<u8>> = sqlx::query_scalar("SELECT text_sealed FROM telegram_outbox").fetch_one(&s.db).await.unwrap();
	assert!(text.is_none(), "the text is dropped once sent");

	// Past the SLA, one reminder; the operator gets it, once.
	let later = t0() + SignedDuration::from_mins(31);
	s.panel.telegram_access_seen(operator, Some(Role::Operator), "Olga", later).await.unwrap();
	s.panel.telegram_access_seen(muted, Some(Role::Admin), "Mia", later).await.unwrap();
	assert_eq!(s.panel.telegram_fan_out(later, Locale::Ru).await.unwrap(), 2, "the admin muted new leads, not reminders");
	assert_eq!(s.panel.telegram_fan_out(later + SignedDuration::from_mins(5), Locale::Ru).await.unwrap(), 0);
	assert_eq!(s.n.deliver(later).await.unwrap().sent, 2);
	let reminders = texts(&s.mock.calls("sendMessage")[1..]);
	assert!(reminders.iter().all(|t| t.starts_with("Заявка ждёт звонка 31 мин\n")), "{reminders:?}");

	// A payment goes to the admins who asked for it.
	s.panel
		.telegram_set_rules(muted, Role::Admin, &[(panel_core::notify::Rule::PaymentReceived, true)])
		.await
		.unwrap();
	let payment = Payment {
		billed: 12000,
		commission: 2400,
		currency: "EUR".into(),
	};
	s.panel.record_payment(Actor(operator), &brand(), &lead, payment, later).await.unwrap();
	assert_eq!(s.panel.telegram_fan_out(later, Locale::Ru).await.unwrap(), 1);
	s.n.deliver(later + SignedDuration::from_secs(2)).await.unwrap();
	let last = s.mock.calls("sendMessage").pop().unwrap();
	assert_eq!(last["chat_id"], 2);
	assert!(last["text"].as_str().unwrap().starts_with("Оплата получена"));
}

#[tokio::test]
async fn a_silent_source_is_told_to_admins_once_a_day() {
	let Some(s) = setup().await else { return };
	let admin = Uuid::now_v7();
	s.panel
		.add_source("aquafix-site", panel_core::event::SourceKind::Site, [brand()].into_iter().collect())
		.await
		.unwrap()
		.unwrap();
	let added = t0() - SignedDuration::from_hours(25);
	sqlx::query("UPDATE sources SET created_at = $1::timestamptz")
		.bind(added.to_string())
		.execute(&s.db)
		.await
		.unwrap();
	s.link(admin, Role::Admin, 5, t0()).await;
	s.panel.telegram_set_rules(admin, Role::Admin, &[(panel_core::notify::Rule::SourceSilent, true)]).await.unwrap();
	s.mock.clear();
	assert_eq!(s.panel.telegram_fan_out(t0(), Locale::En).await.unwrap(), 1);
	let hour = SignedDuration::from_hours(1);
	// Silent for 25 h at t0: told. At 47 h, still the same day of silence; at 48 h, the next.
	s.panel.telegram_access_seen(admin, Some(Role::Admin), "Ann", t0() + hour * 22).await.unwrap();
	assert_eq!(s.panel.telegram_fan_out(t0() + hour * 22, Locale::En).await.unwrap(), 0, "not twice for one day of silence");
	s.panel.telegram_access_seen(admin, Some(Role::Admin), "Ann", t0() + hour * 23).await.unwrap();
	assert_eq!(s.panel.telegram_fan_out(t0() + hour * 23, Locale::En).await.unwrap(), 1, "the next day of it");
	s.n.deliver(t0() + hour * 23).await.unwrap();
	let told = texts(&s.mock.calls("sendMessage"));
	assert!(told.iter().all(|t| t.starts_with("Source silent for over a day\naquafix-site (site): no events yet")), "{told:?}");
}

// ── the outbox ───────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn the_outbox_keeps_one_a_second_per_chat_and_25_in_all() {
	let Some(s) = setup().await else { return };
	for chat in 1..=30 {
		s.link(Uuid::now_v7(), Role::Operator, chat, t0()).await;
	}
	s.mock.clear();
	for i in 0..2 {
		s.lead(t0() + SignedDuration::from_secs(i)).await;
	}
	assert_eq!(s.panel.telegram_fan_out(t0(), Locale::Ru).await.unwrap(), 60);
	assert_eq!(s.panel.telegram_fan_out(t0(), Locale::Ru).await.unwrap(), 0, "idempotent by (rule, event, chat)");

	// Each pass at its virtual time: how many went, to which chats.
	let mut sent_at: HashMap<i64, Vec<i64>> = HashMap::new();
	let mut total = 0;
	for ms in [0, 500, 999, 1000, 1500, 2000, 2500, 3000, 4000] {
		let before = s.mock.calls("sendMessage").len();
		let done = s.n.deliver(at(ms)).await.unwrap();
		let now_sent = &s.mock.calls("sendMessage")[before..];
		assert_eq!(done.sent, now_sent.len());
		assert!(now_sent.len() <= 25, "at most 25 at {ms} ms");
		for m in now_sent {
			sent_at.entry(m["chat_id"].as_i64().unwrap()).or_default().push(ms);
		}
		total += now_sent.len();
		let per_second: usize = sent_at.values().flatten().filter(|&&t| t > ms - 1000 && t <= ms).count();
		assert!(per_second <= 25, "{per_second} in the second up to {ms} ms");
	}
	assert_eq!(total, 60, "everything went, in the end");
	for (chat, times) in &sent_at {
		assert!(times.windows(2).all(|w| w[1] - w[0] >= 1000), "chat {chat}: {times:?}");
	}
	assert_eq!(sent_at.values().flatten().filter(|&&t| t == 0).count(), 25);
	assert_eq!(sent_at.values().flatten().filter(|&&t| t == 500 || t == 999).count(), 0, "the second is spent");
}

#[tokio::test]
async fn a_429_waits_what_telegram_says_and_a_5xx_backs_off() {
	let Some(s) = setup().await else { return };
	s.link(Uuid::now_v7(), Role::Operator, 1, t0()).await;
	s.mock.clear();
	s.lead(t0()).await;
	s.panel.telegram_fan_out(t0(), Locale::Ru).await.unwrap();

	s.mock.script(
		429,
		json!({"ok": false, "error_code": 429, "description": "Too Many Requests: retry after 7", "parameters": {"retry_after": 7}}),
	);
	assert_eq!(s.n.deliver(at(0)).await.unwrap().retrying, 1);
	assert_eq!(s.n.deliver(at(6_999)).await.unwrap().sent, 0, "not before retry_after");
	s.mock.script(502, json!({"ok": false, "error_code": 502, "description": "Bad Gateway"}));
	assert_eq!(s.n.deliver(at(7_000)).await.unwrap().retrying, 1);
	assert_eq!(s.n.deliver(at(7_000 + 19_000)).await.unwrap().sent, 0, "backing off: 20 s after the second failure");
	assert_eq!(s.n.deliver(at(7_000 + 20_000)).await.unwrap().sent, 1);
	assert_eq!(s.outbox().await, [(1, "new_lead".to_owned(), "sent".to_owned(), 3)]);
}

#[tokio::test]
async fn a_blocked_bot_marks_the_chat_dead() {
	let Some(s) = setup().await else { return };
	let user = Uuid::now_v7();
	s.link(user, Role::Operator, 1, t0()).await;
	s.mock.clear();
	s.lead(t0()).await;
	s.lead(t0()).await;
	assert_eq!(s.panel.telegram_fan_out(t0(), Locale::Ru).await.unwrap(), 2);

	s.mock
		.script(403, json!({"ok": false, "error_code": 403, "description": "Forbidden: bot was blocked by the user"}));
	assert_eq!(s.n.deliver(at(0)).await.unwrap().dead, 1);
	let states: Vec<String> = s.outbox().await.into_iter().map(|(_, _, state, _)| state).collect();
	assert_eq!(states, ["dead", "dead"], "and what it was still owed is given up too");
	let settings = s.panel.telegram_settings(user, Role::Operator).await.unwrap();
	assert!(settings.linked && settings.blocked);

	s.lead(at(5_000)).await;
	assert_eq!(s.panel.telegram_fan_out(at(5_000), Locale::Ru).await.unwrap(), 0, "nothing more to a dead chat");
	assert_eq!(s.n.deliver(at(10_000)).await.unwrap(), Default::default());

	// Linking again brings it back.
	s.link(user, Role::Operator, 1, at(20_000)).await;
	assert!(!s.panel.telegram_settings(user, Role::Operator).await.unwrap().blocked);
}

// ── the buttons ──────────────────────────────────────────────────────────────────────────

/// Delivers the one due message; its `(message_id, [take data, no-answer data], text)`.
async fn delivered(s: &Setup, now: Timestamp) -> (i64, Vec<String>, String) {
	let before = s.mock.calls("sendMessage").len();
	assert_eq!(s.n.deliver(now).await.unwrap().sent, 1);
	let sent = s.mock.calls("sendMessage")[before].clone();
	let mid: i64 = sqlx::query_scalar("SELECT tg_message_id FROM telegram_outbox WHERE state = 'sent' ORDER BY sent_at DESC, id DESC LIMIT 1")
		.fetch_one(&s.db)
		.await
		.unwrap();
	let data = sent["reply_markup"]["inline_keyboard"]
		.as_array()
		.unwrap()
		.iter()
		.map(|row| row[0]["callback_data"].as_str().unwrap().to_owned())
		.collect();
	(mid, data, sent["text"].as_str().unwrap().to_owned())
}

fn press(id: &str, chat: i64, message_id: i64, data: &str, text: &str) -> Update {
	Update::Callback {
		id: id.into(),
		chat_id: chat,
		message_id,
		data: data.into(),
		text: text.into(),
	}
}

fn answers(s: &Setup) -> Vec<String> {
	s.mock.calls("answerCallbackQuery").iter().map(|c| c["text"].as_str().unwrap().to_owned()).collect()
}

#[tokio::test]
async fn a_button_records_its_event_once() {
	let Some(s) = setup().await else { return };
	let user = Uuid::now_v7();
	s.link(user, Role::Operator, 1, t0()).await;
	s.session(user, Some(Role::Operator), "Olga").await;
	let lead = s.lead(t0()).await;
	s.panel.telegram_fan_out(t0(), Locale::Ru).await.unwrap();
	let (mid, data, text) = delivered(&s, t0()).await;
	let (take, no_answer) = (&data[0], &data[1]);

	// "No answer": an attempt and its outcome, once however often the update arrives.
	for _ in 0..2 {
		s.n.handle(press("cb-1", 1, mid, no_answer, &text), at(1_000)).await.unwrap();
	}
	assert_eq!((s.events_of(&lead, "call.attempted").await, s.events_of(&lead, "call.logged").await), (1, 1));
	assert_eq!(answers(&s), ["Записано.", "Уже записано."]);
	let edits = s.mock.calls("editMessageText");
	assert_eq!(edits.len(), 1, "edited once");
	assert!(edits[0]["text"].as_str().unwrap().ends_with("\n\nНе дозвонился: Olga"));
	let kept = &edits[0]["reply_markup"]["inline_keyboard"];
	assert_eq!(kept.as_array().unwrap().len(), 1, "only Take is left");

	// "Take": lead.contacted, written as the operator API writes it — once, even pressed again
	// (a new callback id) or with the button still on an older copy of the message.
	s.n.handle(press("cb-2", 1, mid, take, &text), at(2_000)).await.unwrap();
	s.n.handle(press("cb-2", 1, mid, take, &text), at(2_000)).await.unwrap();
	s.n.handle(press("cb-3", 1, mid, take, &text), at(3_000)).await.unwrap();
	assert_eq!(s.events_of(&lead, "lead.contacted").await, 1);
	let (view, events) = s.panel.lead_card(&brand(), &lead, Pii::Withhold, at(3_000)).await.unwrap().unwrap();
	assert_eq!(view.row.stage, Stage::Contacted);
	let contacted = events.iter().find(|e| e.row.r#type == "lead.contacted").unwrap();
	assert_eq!(
		(contacted.row.source_kind.as_str(), contacted.row.source_id.clone()),
		("panel", user.to_string()),
		"entered by the user"
	);
	assert_eq!(answers(&s)[2..], ["Записано.", "Уже записано.", "Уже записано."]);
	let edits = s.mock.calls("editMessageText");
	assert!(edits[1]["text"].as_str().unwrap().ends_with("\n\nВзял: Olga"));
	assert_eq!(edits[1]["reply_markup"]["inline_keyboard"], json!([]), "the buttons are gone");
}

#[tokio::test]
async fn a_button_needs_access_now() {
	let Some(s) = setup().await else { return };
	let user = Uuid::now_v7();
	s.link(user, Role::Operator, 1, t0()).await;
	let lead = s.lead(t0()).await;
	s.panel.telegram_fan_out(t0(), Locale::Ru).await.unwrap();
	let (mid, data, text) = delivered(&s, t0()).await;

	// No panel session to ask concierge with: nothing is written on the link's old answer.
	s.n.handle(press("cb-1", 1, mid, &data[0], &text), at(1_000)).await.unwrap();
	// A session, but concierge says the grant is gone.
	s.session(user, None, "Olga").await;
	s.n.handle(press("cb-2", 1, mid, &data[0], &text), at(2_000)).await.unwrap();
	assert_eq!(answers(&s), ["Откройте панель, чтобы подтвердить доступ, и нажмите снова.", "Нет доступа к панели."]);
	assert_eq!(s.events_of(&lead, "lead.contacted").await, 0);
	assert!(s.mock.calls("editMessageText").is_empty());

	// And the user gets nothing more until concierge says otherwise.
	s.lead(at(3_000)).await;
	assert_eq!(s.panel.telegram_fan_out(at(3_000), Locale::Ru).await.unwrap(), 0);
}

#[tokio::test]
async fn a_forged_button_is_refused() {
	let Some(s) = setup().await else { return };
	let user = Uuid::now_v7();
	s.link(user, Role::Operator, 1, t0()).await;
	s.session(user, Some(Role::Operator), "Olga").await;
	let lead = s.lead(t0()).await;
	s.panel.telegram_fan_out(t0(), Locale::Ru).await.unwrap();
	let (mid, data, text) = delivered(&s, t0()).await;
	let take = &data[0];
	let (outbox, mac) = {
		let mut parts = take.splitn(3, '.');
		(parts.nth(1).unwrap().parse::<i64>().unwrap(), take.rsplit('.').next().unwrap().to_owned())
	};

	let forged = [
		format!("t.{}.{mac}", outbox + 1),        // another message, the same MAC
		format!("n.{outbox}.{mac}"),              // another button
		format!("t.{outbox}.{}", "0".repeat(20)), // a made-up MAC
		"t.1".to_owned(),
		"garbage".to_owned(),
	];
	for (i, data) in forged.iter().enumerate() {
		s.n.handle(press(&format!("cb-{i}"), 1, mid, data, &text), at(1_000)).await.unwrap();
	}
	// The real data, pressed from another chat: bound to the chat it was sent to.
	s.n.handle(press("cb-x", 2, mid, take, &text), at(1_000)).await.unwrap();
	assert!(answers(&s).iter().all(|a| a == "Кнопка недействительна."), "{:?}", answers(&s));
	assert_eq!(answers(&s).len(), forged.len() + 1);
	assert_eq!(s.events_of(&lead, "lead.contacted").await, 0);
	assert!(s.mock.calls("editMessageText").is_empty());
}
