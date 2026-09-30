//! Telegram notifications (spec §8) without the I/O: which rules there are and who may get
//! each, what a message says, what its buttons carry, and how a failed delivery is retried.
//!
//! The database and the Bot API are the engine's and the server's; this decides.

use std::{fmt, str::FromStr};

use hmac::{Hmac, KeyInit, Mac};
use jiff::{SignedDuration, Timestamp};
use sha2::Sha256;
use subtle::ConstantTimeEq;

use crate::{Invalid, role::Role};

/// What a user may be notified of. Each is on or off per user; [`Rule::on_by_default`] is
/// what a user who never chose gets.
///
/// Not yet, for want of a source: a review of 3★ or less (review_archive's events), a stage
/// of the funnel dropping more than X % below its 4-week mean (the GBP and PostHog imports),
/// a Grafana alert. Each will be a variant here, a fan-out in the engine, and a text below;
/// the outbox, the pacing and the linking do not change.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Rule {
	/// A lead came in: brand, location, what is needed and the phone, with buttons.
	NewLead,
	/// A lead has waited past [`crate::funnel::CONTACT_SLA`] for its first contact: once.
	ContactOverdue,
	/// A payment was recorded.
	PaymentReceived,
	/// A source sent nothing for [`SOURCE_SILENCE`]: at most once a day.
	SourceSilent,
}

impl Rule {
	pub const ALL: [Self; 4] = [Self::NewLead, Self::ContactOverdue, Self::PaymentReceived, Self::SourceSilent];

	pub fn as_str(self) -> &'static str {
		match self {
			Self::NewLead => "new_lead",
			Self::ContactOverdue => "contact_overdue",
			Self::PaymentReceived => "payment_received",
			Self::SourceSilent => "source_silent",
		}
	}

	/// On for a user who never chose.
	pub fn on_by_default(self) -> bool {
		match self {
			Self::NewLead | Self::ContactOverdue => true,
			Self::PaymentReceived | Self::SourceSilent => false,
		}
	}

	/// Whether a role gets this rule's messages at all: leads are everyone's work; money and
	/// the sources are the admins'.
	pub fn open_to(self, role: Role) -> bool {
		match self {
			Self::NewLead | Self::ContactOverdue => role.edits_leads(),
			Self::PaymentReceived | Self::SourceSilent => role.manages_sources(),
		}
	}
}

impl fmt::Display for Rule {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.write_str(self.as_str())
	}
}

impl FromStr for Rule {
	type Err = Invalid;

	fn from_str(s: &str) -> Result<Self, Invalid> {
		Self::ALL
			.into_iter()
			.find(|r| r.as_str() == s)
			.ok_or_else(|| Invalid::new(format!("{s:?} is not one of new_lead, contact_overdue, payment_received, source_silent")))
	}
}

/// How long a source may send nothing before the admins hear of it.
pub const SOURCE_SILENCE: SignedDuration = SignedDuration::from_hours(24);

/// The language of the bot's messages (one per deployment, `TELEGRAM_LOCALE`).
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Locale {
	#[default]
	Ru,
	En,
}

impl FromStr for Locale {
	type Err = Invalid;

	fn from_str(s: &str) -> Result<Self, Invalid> {
		match s.trim() {
			"ru" => Ok(Self::Ru),
			"en" => Ok(Self::En),
			other => Err(Invalid::new(format!("TELEGRAM_LOCALE {other:?} is not one of ru, en"))),
		}
	}
}

/// A button under a lead's message.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Button {
	/// "Взял": the lead is contacted (`lead.contacted`).
	Take,
	/// "Не дозвонился": a call attempted and not answered (`call.attempted` + `call.logged`).
	NoAnswer,
}

impl Button {
	fn code(self) -> &'static str {
		match self {
			Self::Take => "t",
			Self::NoAnswer => "n",
		}
	}

	fn from_code(c: &str) -> Option<Self> {
		match c {
			"t" => Some(Self::Take),
			"n" => Some(Self::NoAnswer),
			_ => None,
		}
	}

	pub fn as_str(self) -> &'static str {
		match self {
			Self::Take => "take",
			Self::NoAnswer => "no_answer",
		}
	}

	/// The button [`Self::as_str`] names.
	pub fn from_name(name: &str) -> Option<Self> {
		[Self::Take, Self::NoAnswer].into_iter().find(|b| b.as_str() == name)
	}

	pub fn label(self, locale: Locale) -> &'static str {
		match (self, locale) {
			(Self::Take, Locale::Ru) => "Взял",
			(Self::Take, Locale::En) => "Taken",
			(Self::NoAnswer, Locale::Ru) => "Не дозвонился",
			(Self::NoAnswer, Locale::En) => "No answer",
		}
	}
}

/// Bytes of HMAC in a button's data: 80 bits, far past guessing within Telegram's rate of
/// callbacks, and short enough for its 64-byte limit.
const CALLBACK_MAC_LEN: usize = 10;

fn callback_mac(key: &[u8; 32], chat_id: i64, message: i64, button: Button) -> [u8; CALLBACK_MAC_LEN] {
	// HMAC takes a key of any length, so this cannot fail.
	let mut mac = <Hmac<Sha256> as KeyInit>::new_from_slice(key).expect("HMAC accepts any key length");
	mac.update(b"sa-panel/tg-callback/v1/");
	mac.update(&chat_id.to_be_bytes());
	mac.update(&message.to_be_bytes());
	mac.update(button.code().as_bytes());
	let full = mac.finalize().into_bytes();
	let mut out = [0u8; CALLBACK_MAC_LEN];
	out.copy_from_slice(&full[..CALLBACK_MAC_LEN]);
	out
}

/// A button's `callback_data`: `<button>.<outbox message id>.<mac>`. The message id is all it
/// names; the MAC, under a key derived from `PANEL_DATA_KEY` and bound to the chat, is what
/// makes it the panel's — Telegram hands the data back verbatim, and anyone who can talk to
/// the bot can send any data they like.
pub fn callback_data(key: &[u8; 32], chat_id: i64, message: i64, button: Button) -> String {
	format!("{}.{message}.{}", button.code(), hex::encode(callback_mac(key, chat_id, message, button)))
}

/// The message and button a callback names, if its data is the panel's for this chat.
pub fn parse_callback(key: &[u8; 32], chat_id: i64, data: &str) -> Option<(i64, Button)> {
	let mut parts = data.splitn(3, '.');
	let (button, message, mac) = (parts.next()?, parts.next()?, parts.next()?);
	let button = Button::from_code(button)?;
	let message: i64 = message.parse().ok()?;
	let mac = hex::decode(mac).ok()?;
	let want = callback_mac(key, chat_id, message, button);
	(mac.len() == want.len() && bool::from(mac.as_slice().ct_eq(&want))).then_some((message, button))
}

/// A lead as a message shows it. The PII fields are set only for a role that sees PII.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct LeadNote {
	pub brand: String,
	pub location: Option<String>,
	pub name: Option<String>,
	pub need: Option<String>,
	pub phone: Option<String>,
}

/// What a message is about.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Note {
	NewLead(LeadNote),
	ContactOverdue {
		lead: LeadNote,
		waiting: SignedDuration,
	},
	PaymentReceived {
		brand: String,
		lead: String,
		billed: i64,
		commission: i64,
		currency: String,
	},
	SourceSilent {
		key_id: String,
		kind: String,
		last: Option<Timestamp>,
	},
}

impl Note {
	pub fn rule(&self) -> Rule {
		match self {
			Self::NewLead(_) => Rule::NewLead,
			Self::ContactOverdue { .. } => Rule::ContactOverdue,
			Self::PaymentReceived { .. } => Rule::PaymentReceived,
			Self::SourceSilent { .. } => Rule::SourceSilent,
		}
	}

	/// The buttons under it: the lead's two actions for the lead rules, none otherwise.
	pub fn buttons(&self) -> &'static [Button] {
		match self {
			Self::NewLead(_) | Self::ContactOverdue { .. } => &[Button::Take, Button::NoAnswer],
			Self::PaymentReceived { .. } | Self::SourceSilent { .. } => &[],
		}
	}

	/// Plain text (sent without a parse mode, so nothing a customer typed is markup).
	pub fn text(&self, locale: Locale) -> String {
		let ru = locale == Locale::Ru;
		match self {
			Self::NewLead(lead) => lead_text(if ru { "Новая заявка" } else { "New lead" }, lead, locale),
			Self::ContactOverdue { lead, waiting } => {
				let mins = waiting.as_mins().max(0);
				let head = if ru {
					format!("Заявка ждёт звонка {mins} мин")
				} else {
					format!("Lead waiting for a call for {mins} min")
				};
				lead_text(&head, lead, locale)
			}
			Self::PaymentReceived {
				brand,
				lead,
				billed,
				commission,
				currency,
			} => {
				let (billed, commission) = (money(*billed), money(*commission));
				if ru {
					format!("Оплата получена\n{brand} · {lead}\nСумма: {billed} {currency}\nКомиссия: {commission} {currency}")
				} else {
					format!("Payment received\n{brand} · {lead}\nBilled: {billed} {currency}\nCommission: {commission} {currency}")
				}
			}
			Self::SourceSilent { key_id, kind, last } => {
				let since = match (last, ru) {
					(Some(at), true) => format!("последнее событие {}", minute(*at)),
					(Some(at), false) => format!("last event {}", minute(*at)),
					(None, true) => "событий не было".to_owned(),
					(None, false) => "no events yet".to_owned(),
				};
				if ru {
					format!("Источник молчит больше суток\n{key_id} ({kind}): {since}")
				} else {
					format!("Source silent for over a day\n{key_id} ({kind}): {since}")
				}
			}
		}
	}
}

fn lead_text(head: &str, lead: &LeadNote, locale: Locale) -> String {
	let ru = locale == Locale::Ru;
	let mut out = match &lead.location {
		Some(location) => format!("{head}\n{} · {location}", lead.brand),
		None => format!("{head}\n{}", lead.brand),
	};
	let fields = [
		(if ru { "Имя" } else { "Name" }, &lead.name),
		(if ru { "Нужно" } else { "Needs" }, &lead.need),
		(if ru { "Телефон" } else { "Phone" }, &lead.phone),
	];
	for (label, value) in fields {
		if let Some(v) = value.as_deref().map(str::trim).filter(|v| !v.is_empty()) {
			out.push_str(&format!("\n{label}: {v}"));
		}
	}
	out
}

/// Minor units as the amount a person reads: whole when it is whole, else with its cents.
/// Every currency the panel takes has two decimals.
fn money(minor: i64) -> String {
	let sign = if minor < 0 { "-" } else { "" };
	let abs = minor.unsigned_abs();
	if abs.is_multiple_of(100) {
		format!("{sign}{}", abs / 100)
	} else {
		format!("{sign}{}.{:02}", abs / 100, abs % 100)
	}
}

/// UTC, to the minute.
fn minute(at: Timestamp) -> String {
	at.strftime("%Y-%m-%d %H:%M UTC").to_string()
}

/// The line added to a lead's message once a button was pressed; the buttons left under it
/// are [`after_press`].
pub fn pressed_line(button: Button, who: &str, locale: Locale) -> String {
	match (button, locale) {
		(Button::Take, Locale::Ru) => format!("Взял: {who}"),
		(Button::Take, Locale::En) => format!("Taken by: {who}"),
		(Button::NoAnswer, Locale::Ru) => format!("Не дозвонился: {who}"),
		(Button::NoAnswer, Locale::En) => format!("No answer: {who}"),
	}
}

/// The buttons a message keeps after one was pressed: none once taken; "Take" still, after
/// a call nobody answered.
pub fn after_press(button: Button) -> &'static [Button] {
	match button {
		Button::Take => &[],
		Button::NoAnswer => &[Button::Take],
	}
}

/// What the bot says back, by occasion.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Reply {
	Linked,
	LinkInvalid,
	Help,
	Unlinked,
	NotLinked,
	/// The button is not one the panel made for this chat.
	BadButton,
	/// The button's user has no live panel session to ask concierge with.
	OpenPanel,
	NoAccess,
	/// concierge could not be asked.
	TryLater,
	/// Recorded.
	Done,
	/// Pressed before; nothing more recorded.
	AlreadyDone,
	/// The lead is gone from the projection (a rebuild dropped it).
	LeadGone,
}

impl Reply {
	pub fn text(self, locale: Locale) -> &'static str {
		let ru = locale == Locale::Ru;
		match self {
			Self::Linked if ru => "Telegram подключён к панели Service-Arb. Уведомления настраиваются в профиле панели.",
			Self::Linked => "Telegram is linked to the Service-Arb panel. Choose notifications in your panel profile.",
			Self::LinkInvalid if ru => "Ссылка недействительна или устарела. Откройте профиль в панели и подключите Telegram заново.",
			Self::LinkInvalid => "This link is invalid or has expired. Open your panel profile and link Telegram again.",
			Self::Help if ru => "Это бот панели Service-Arb. Подключение — кнопкой «Подключить Telegram» в профиле панели. /stop отключает уведомления.",
			Self::Help => "This is the Service-Arb panel's bot. Link it with \"Connect Telegram\" in your panel profile. /stop turns notifications off.",
			Self::Unlinked if ru => "Уведомления отключены. Подключить снова можно в профиле панели.",
			Self::Unlinked => "Notifications are off. You can link again from your panel profile.",
			Self::NotLinked if ru => "Этот чат не подключён к панели.",
			Self::NotLinked => "This chat is not linked to the panel.",
			Self::BadButton if ru => "Кнопка недействительна.",
			Self::BadButton => "This button is not valid.",
			Self::OpenPanel if ru => "Откройте панель, чтобы подтвердить доступ, и нажмите снова.",
			Self::OpenPanel => "Open the panel to confirm your access, then press again.",
			Self::NoAccess if ru => "Нет доступа к панели.",
			Self::NoAccess => "No access to the panel.",
			Self::TryLater if ru => "Не удалось проверить доступ. Попробуйте через минуту.",
			Self::TryLater => "Could not check your access. Try again in a minute.",
			Self::Done if ru => "Записано.",
			Self::Done => "Recorded.",
			Self::AlreadyDone if ru => "Уже записано.",
			Self::AlreadyDone => "Already recorded.",
			Self::LeadGone if ru => "Заявка не найдена.",
			Self::LeadGone => "Lead not found.",
		}
	}
}

// ── delivery ────────────────────────────────────────────────────────────────────────────

/// At most one message a second to a chat (Telegram's limit for a private chat).
pub const PER_CHAT_GAP: SignedDuration = SignedDuration::from_secs(1);

/// At most this many messages in any second, across every chat and every replica (under the
/// Bot API's 30).
pub const GLOBAL_PER_SECOND: i64 = 25;

/// Tries before a message is given up on: with [`backoff`], about an hour — past that a lead
/// notice is news nobody needs.
pub const MAX_ATTEMPTS: i32 = 10;

/// How a try to send ended.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Failure {
	/// 429: Telegram says when to try again.
	RetryAfter(SignedDuration),
	/// 5xx, a timeout, no connection: tried again later.
	Transient(String),
	/// 403: the user blocked the bot. The chat is dead until they link again.
	Blocked,
	/// Any other refusal (400: chat not found, text too long): trying again changes nothing.
	Refused(String),
}

/// What becomes of a message after a failed try.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Next {
	Retry {
		at: Timestamp,
	},
	/// Given up on: the tries ran out, or no try can succeed.
	Dead,
	/// Given up on, and the chat with it.
	ChatDead,
}

/// The wait after the `n`th failed try (1-based): 10 s, doubling, at most 15 minutes.
pub fn backoff(n: i32) -> SignedDuration {
	let doublings = u32::try_from(n.saturating_sub(1)).unwrap_or(0).min(16);
	SignedDuration::from_secs(10)
		.checked_mul(1 << doublings)
		.unwrap_or(SignedDuration::MAX)
		.min(SignedDuration::from_mins(15))
}

/// The fate of a message whose `attempts`th try (this one included) failed with `failure`.
pub fn after_failure(attempts: i32, failure: &Failure, now: Timestamp) -> Next {
	match failure {
		Failure::Blocked => Next::ChatDead,
		Failure::Refused(_) => Next::Dead,
		Failure::RetryAfter(_) | Failure::Transient(_) if attempts >= MAX_ATTEMPTS => Next::Dead,
		Failure::RetryAfter(wait) => Next::Retry {
			at: now.saturating_add((*wait).max(PER_CHAT_GAP)).unwrap_or(Timestamp::MAX),
		},
		Failure::Transient(_) => Next::Retry {
			at: now.saturating_add(backoff(attempts)).unwrap_or(Timestamp::MAX),
		},
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	const KEY: [u8; 32] = [7; 32];

	#[test]
	fn rules_round_trip_and_are_scoped() {
		for r in Rule::ALL {
			assert_eq!(r.as_str().parse::<Rule>().unwrap(), r);
		}
		assert!("review_low".parse::<Rule>().is_err(), "not yet");
		assert_eq!(Rule::ALL.iter().filter(|r| r.on_by_default()).count(), 2);
		assert!(Rule::NewLead.open_to(Role::Operator) && Rule::ContactOverdue.open_to(Role::Operator));
		assert!(!Rule::PaymentReceived.open_to(Role::Operator) && !Rule::SourceSilent.open_to(Role::Operator));
		assert!(Rule::ALL.iter().all(|r| r.open_to(Role::Admin)));
	}

	#[test]
	fn callbacks_are_bound_to_chat_message_and_button() {
		let data = callback_data(&KEY, 42, 1001, Button::Take);
		assert!(data.len() <= 64, "{data}");
		assert_eq!(parse_callback(&KEY, 42, &data), Some((1001, Button::Take)));
		assert_eq!(parse_callback(&KEY, 43, &data), None, "another chat");
		assert_eq!(parse_callback(&[8; 32], 42, &data), None, "another key");
		assert_eq!(parse_callback(&KEY, 42, &data.replacen("t.1001", "t.1002", 1)), None, "another message");
		assert_eq!(parse_callback(&KEY, 42, &data.replacen("t.", "n.", 1)), None, "another button");
		assert_eq!(parse_callback(&KEY, 42, "t.1001"), None);
		assert_eq!(parse_callback(&KEY, 42, "t.1001.zz"), None);
		assert_eq!(parse_callback(&KEY, 42, ""), None);
		let big = callback_data(&KEY, i64::MIN, i64::MAX, Button::NoAnswer);
		assert!(big.len() <= 64, "{big}");
	}

	#[test]
	fn texts() {
		let lead = LeadNote {
			brand: "aquafix".into(),
			location: Some("paris-11".into()),
			name: None,
			need: Some("leaking tap".into()),
			phone: Some("+33 6 00 00 00 00".into()),
		};
		assert_eq!(
			Note::NewLead(lead.clone()).text(Locale::Ru),
			"Новая заявка\naquafix · paris-11\nНужно: leaking tap\nТелефон: +33 6 00 00 00 00"
		);
		let withheld = LeadNote {
			need: None,
			phone: None,
			..lead.clone()
		};
		assert_eq!(Note::NewLead(withheld).text(Locale::En), "New lead\naquafix · paris-11");
		let overdue = Note::ContactOverdue {
			lead,
			waiting: SignedDuration::from_mins(47),
		};
		assert!(overdue.text(Locale::Ru).starts_with("Заявка ждёт звонка 47 мин\n"));
		assert_eq!(overdue.buttons(), [Button::Take, Button::NoAnswer]);
		let paid = Note::PaymentReceived {
			brand: "aquafix".into(),
			lead: "L-1".into(),
			billed: 12050,
			commission: 3000,
			currency: "EUR".into(),
		};
		assert_eq!(paid.text(Locale::En), "Payment received\naquafix · L-1\nBilled: 120.50 EUR\nCommission: 30 EUR");
		assert!(paid.buttons().is_empty());
		let silent = Note::SourceSilent {
			key_id: "aquafix-site".into(),
			kind: "site".into(),
			last: Some("2026-09-29T08:15:30Z".parse().unwrap()),
		};
		assert_eq!(
			silent.text(Locale::Ru),
			"Источник молчит больше суток\naquafix-site (site): последнее событие 2026-09-29 08:15 UTC"
		);
		assert_eq!(money(-5), "-0.05");
	}

	#[test]
	fn failures() {
		let now: Timestamp = "2026-09-30T10:00:00Z".parse().unwrap();
		let s = SignedDuration::from_secs;
		assert_eq!(after_failure(1, &Failure::RetryAfter(s(7)), now), Next::Retry { at: now + s(7) });
		assert_eq!(
			after_failure(1, &Failure::RetryAfter(s(0)), now),
			Next::Retry { at: now + s(1) },
			"never sooner than the chat's gap"
		);
		assert_eq!(after_failure(1, &Failure::Transient("502".into()), now), Next::Retry { at: now + s(10) });
		assert_eq!(after_failure(3, &Failure::Transient("502".into()), now), Next::Retry { at: now + s(40) });
		assert_eq!(after_failure(MAX_ATTEMPTS, &Failure::Transient("502".into()), now), Next::Dead);
		assert_eq!(after_failure(MAX_ATTEMPTS, &Failure::RetryAfter(s(1)), now), Next::Dead);
		assert_eq!(after_failure(1, &Failure::Blocked, now), Next::ChatDead);
		assert_eq!(after_failure(1, &Failure::Refused("chat not found".into()), now), Next::Dead);
		assert_eq!(backoff(100), SignedDuration::from_mins(15));
		let total: i64 = (1..MAX_ATTEMPTS).map(|n| backoff(n).as_secs()).sum();
		assert!((1800..=2 * 3600).contains(&total), "{total}");
	}
}
