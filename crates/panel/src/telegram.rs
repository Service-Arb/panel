//! Telegram notifications in private chats (spec §8): linking a chat to a panel user, the
//! rules' fan-out into the outbox, delivering the outbox, and what the bot does with the
//! updates it receives — `/start <token>`, `/stop` and the buttons under a lead.
//!
//! ```text
//! POST /api/v1/telegram/link ─ token (256 bits; its hash, 10 min) ─ t.me/<bot>?start=<token>
//!                         /start <token> in a private chat ─ link user ⇄ chat
//! journal ─ fan-out (a rule × a new lead / overdue lead / payment / silent source) ─ outbox
//! outbox ─ claim (1/s per chat, ≤ 25/s in all, across replicas) ─ sendMessage ─ sent | retry | dead
//! button ─ signed data ─ the chat's user, access asked of concierge now ─ event, as the API writes it
//! ```
//!
//! **Who has access.** The panel learns a role only from concierge's `GetMe`, which takes
//! the user's own access token. A link keeps the role concierge last confirmed and when:
//! the `/api/v1` gate records every answer it gets, and [`Notifier::recheck_access`] asks
//! again through the user's newest panel session for a link not confirmed in
//! [`RECHECK_AFTER`]. A message goes only to a link confirmed within [`ACCESS_TTL`], so a
//! grant revoked at concierge stops them within about [`RECHECK_AFTER`], and concierge being
//! unreachable stops them after [`ACCESS_TTL`] — PII does not flow on a stale answer. A
//! button asks concierge at the moment it is pressed, and without a live panel session to
//! ask with, the user is told to open the panel: nothing is written on a cached answer.
//!
//! The Bot API is the server's; this sees it through [`Bot`], and concierge through
//! [`Directory`] and [`Refresher`].

use std::future::Future;

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use eyre::WrapErr;
use futures::{StreamExt, TryStreamExt};
use jiff::{SignedDuration, Timestamp};
use panel_core::{
	funnel::CONTACT_SLA,
	ids::{BrandId, LeadId},
	notify::{self, Button, Failure, LeadNote, Locale, Next, Note, Reply, Rule},
	role::Role,
};
use serde_json::Value;
use sha2::{Digest, Sha256};
use uuid::Uuid;
use zeroize::Zeroizing;

use crate::{
	Panel,
	operator::{ActionError, Actor, StageMove},
	seal::{pii_aad, telegram_text_aad},
	session::{Refresher, SessionError},
	store::telegram::{self as db, LeadCandidate, NewMessage},
};

/// How long a link token may wait for its `/start`.
pub const LINK_TTL: SignedDuration = SignedDuration::from_mins(10);

/// Nothing is sent to a user whose access concierge has not confirmed for this long.
pub const ACCESS_TTL: SignedDuration = SignedDuration::from_hours(1);

/// A link not confirmed for this long is asked about again.
pub const RECHECK_AFTER: SignedDuration = SignedDuration::from_mins(15);

/// How long a replica sending a message holds it: past the Bot API's timeout, so it lapses
/// only when the replica is gone.
pub const SEND_LEASE: SignedDuration = SignedDuration::from_secs(60);

/// A lead is told of as new while its creation is younger than this; older ones, when the
/// bot starts late, are left to the SLA reminder.
const NEW_LEAD_WINDOW: SignedDuration = SignedDuration::from_hours(1);

/// A lead is reminded of while it has been overdue for less than this.
const OVERDUE_WINDOW: SignedDuration = SignedDuration::from_hours(6);

/// A payment is told of while its record is younger than this.
const PAYMENT_WINDOW: SignedDuration = SignedDuration::from_hours(24);

/// Fan-out marks and finished messages are kept this long; the windows above are shorter.
const KEEP: SignedDuration = SignedDuration::from_hours(24 * 7);

/// Candidates taken per rule per pass.
const BATCH: i64 = 50;

/// Messages sent at once in a pass.
const PARALLEL_SENDS: usize = 8;

/// Links re-confirmed per pass.
const RECHECKS: i64 = 20;

/// A button under a message: its label and its signed data.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InlineButton {
	pub label: &'static str,
	pub data: String,
}

/// The Bot API, as the notifier needs it. Every call answers the failure as
/// [`notify::Failure`], so the outbox can decide what becomes of the message.
pub trait Bot: Sync {
	/// Sends a plain-text message to a chat; the id Telegram gave it.
	fn send(&self, chat_id: i64, text: &str, buttons: &[InlineButton]) -> impl Future<Output = Result<i64, Failure>> + Send;
	/// Replaces a message's text and buttons (none: the keyboard is removed).
	fn edit(&self, chat_id: i64, message_id: i64, text: &str, buttons: &[InlineButton]) -> impl Future<Output = Result<(), Failure>> + Send;
	/// Answers a button press, with a short notice the user sees.
	fn answer(&self, callback_id: &str, text: &str) -> impl Future<Output = Result<(), Failure>> + Send;
}

/// Who concierge says the holder of an access token is, as the panel reads it.
#[derive(Clone, Debug)]
pub struct Identity {
	pub user_id: Uuid,
	/// `None`: concierge knows the user, and the panel does not let them in.
	pub role: Option<Role>,
	/// preferred_name, or the email when there is none.
	pub display_name: String,
}

#[derive(Debug, thiserror::Error)]
pub enum DirectoryError {
	/// The token is refused: revoked, expired, the user held or disabled.
	#[error("concierge refused the token")]
	Refused,
	#[error("concierge is unavailable: {0}")]
	Unavailable(String),
	#[error("concierge failed: {0}")]
	Failed(String),
}

/// concierge's `GetMe`: the port the notifier confirms a user's access through.
pub trait Directory: Sync {
	fn me(&self, access: &str) -> impl Future<Output = Result<Identity, DirectoryError>> + Send;
}

/// A chat an update came from.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Chat {
	pub id: i64,
	/// A one-to-one chat with the bot; groups and channels are never linked.
	pub private: bool,
}

/// What the bot received, as far as the panel cares.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Update {
	/// `/start`, with the payload of a `t.me/<bot>?start=<payload>` link when there is one.
	Start {
		chat: Chat,
		payload: Option<String>,
	},
	Stop {
		chat: Chat,
	},
	/// Any other message.
	Other {
		chat: Chat,
	},
	/// A button pressed under one of the bot's messages.
	Callback {
		id: String,
		chat_id: i64,
		message_id: i64,
		data: String,
		/// The message's text as the user sees it, to extend with who pressed.
		text: String,
	},
}

/// What concierge said of a user just now.
#[derive(Clone, Debug)]
pub enum Access {
	Granted(Role, String),
	/// No live panel session to ask with, or concierge refused its token.
	NoSession,
	/// concierge says the user has no role in the panel.
	Denied,
	Unavailable,
}

/// A user's Telegram settings, as the profile shows them.
#[derive(Clone, Debug)]
pub struct Settings {
	pub linked: bool,
	/// Linked, but the bot was blocked: nothing arrives until they link again.
	pub blocked: bool,
	/// Every rule open to the user's role, with whether it is on.
	pub rules: Vec<(Rule, bool)>,
}

/// What one delivery pass did.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Delivered {
	pub sent: usize,
	pub retrying: usize,
	pub dead: usize,
}

fn link_token_hash(token: &str) -> [u8; 32] {
	let mut h = Sha256::new();
	h.update(b"sa-panel/tg-link/v1/");
	h.update(token.as_bytes());
	h.finalize().into()
}

/// The id a rule fans out under for something that is not a journal event: a UUID (version 8)
/// named by its parts.
fn derived_event_id(parts: &[&str]) -> Uuid {
	let mut h = Sha256::new();
	h.update(b"sa-panel/tg-fanout/v1");
	for p in parts {
		h.update([0]);
		h.update(p.as_bytes());
	}
	let digest = h.finalize();
	let mut bytes = [0u8; 16];
	bytes.copy_from_slice(&digest[..16]);
	uuid::Builder::from_custom_bytes(bytes).into_uuid()
}

fn text_field(pii: &Value, field: &str) -> Option<String> {
	pii.get(field).and_then(Value::as_str).map(str::to_owned)
}

impl Panel {
	/// A one-time token for `t.me/<bot>?start=<token>`, for a signed-in user: 256 random bits,
	/// base64url (the 43 characters a start payload may hold); only its hash is stored, for
	/// [`LINK_TTL`].
	pub async fn telegram_link_token(&self, user: Uuid, role: Role, display: &str, now: Timestamp) -> eyre::Result<Zeroizing<String>> {
		let mut raw = Zeroizing::new([0u8; 32]);
		getrandom::fill(raw.as_mut_slice()).map_err(|e| eyre::eyre!("the OS random source failed: {e}"))?;
		let token = Zeroizing::new(URL_SAFE_NO_PAD.encode(raw.as_slice()));
		let mut conn = self.store.pool().acquire().await.wrap_err("a connection for a link token")?;
		db::insert_link_token(&mut conn, &link_token_hash(&token), user, role, display, now, now + LINK_TTL).await?;
		Ok(token)
	}

	/// Unlinks the user's chat; `false` when there was none.
	pub async fn telegram_unlink(&self, user: Uuid) -> eyre::Result<bool> {
		let mut conn = self.store.pool().acquire().await.wrap_err("a connection to unlink")?;
		db::unlink_user(&mut conn, user).await
	}

	pub async fn telegram_settings(&self, user: Uuid, role: Role) -> eyre::Result<Settings> {
		let mut conn = self.store.pool().acquire().await.wrap_err("a connection for Telegram settings")?;
		let link = db::link_of_user(&mut conn, user).await?;
		let chosen = db::rules(&mut conn, user).await?;
		let rules = Rule::ALL
			.into_iter()
			.filter(|r| r.open_to(role))
			.map(|r| (r, chosen.iter().find(|(c, _)| *c == r).map_or(r.on_by_default(), |(_, on)| *on)))
			.collect();
		Ok(Settings {
			linked: link.is_some(),
			blocked: link.is_some_and(|l| l.dead),
			rules,
		})
	}

	/// Turns rules on or off for the user. A rule their role does not get is refused, not
	/// stored: it would only mislead the profile.
	pub async fn telegram_set_rules(&self, user: Uuid, role: Role, rules: &[(Rule, bool)]) -> Result<(), ActionError> {
		if let Some((r, _)) = rules.iter().find(|(r, _)| !r.open_to(role)) {
			return Err(ActionError::Invalid(panel_core::Invalid::new(format!("{r} is not a rule for the {} role", role.as_str()))));
		}
		let mut tx = self.store.pool().begin().await.wrap_err("beginning a rules change")?;
		for (rule, on) in rules {
			db::set_rule(&mut tx, user, *rule, *on).await?;
		}
		tx.commit().await.wrap_err("committing a rules change")?;
		Ok(())
	}

	/// What concierge just told the `/api/v1` gate of a user: their role in the panel
	/// (`None`: none) and name. Kept on their link, if they have one.
	pub async fn telegram_access_seen(&self, user: Uuid, role: Option<Role>, display: &str, now: Timestamp) -> eyre::Result<()> {
		let mut conn = self.store.pool().acquire().await.wrap_err("a connection to record access")?;
		db::access_seen(&mut conn, user, role, display, now).await
	}

	/// Queues what the rules say is due at `now`: new leads, overdue leads, payments and
	/// silent sources, each told once per recipient. Safe on several replicas at once: each
	/// candidate is claimed, and the outbox refuses a second `(rule, event, chat)`. How many
	/// messages were queued.
	pub async fn telegram_fan_out(&self, now: Timestamp, locale: Locale) -> eyre::Result<usize> {
		let confirmed_since = now - ACCESS_TTL;
		let mut conn = self.store.pool().acquire().await.wrap_err("a connection for the fan-out")?;
		let new = db::new_leads(&mut conn, now - NEW_LEAD_WINDOW, BATCH).await?;
		let overdue = db::overdue_leads(&mut conn, now - CONTACT_SLA - OVERDUE_WINDOW, now - CONTACT_SLA, BATCH).await?;
		let payments = db::new_payments(&mut conn, now - PAYMENT_WINDOW, BATCH).await?;
		let silent = db::silent_sources(&mut conn, now - notify::SOURCE_SILENCE).await?;
		let mut queued = 0;
		for c in &new {
			let lead = Some((c.brand_id.as_str(), c.lead_id.as_str()));
			let note = |role| Ok(Note::NewLead(self.lead_note(c, role)?));
			// Whoever typed the lead in knows of it already.
			queued += self
				.fan_out_one(&mut conn, Rule::NewLead, c.event_id, lead, c.entered_by, note, confirmed_since, now, locale)
				.await?;
		}
		for c in &overdue {
			let lead = Some((c.brand_id.as_str(), c.lead_id.as_str()));
			let waiting = c.created_at.map_or(CONTACT_SLA, |at| now.duration_since(at));
			let note = |role| {
				Ok(Note::ContactOverdue {
					lead: self.lead_note(c, role)?,
					waiting,
				})
			};
			queued += self
				.fan_out_one(&mut conn, Rule::ContactOverdue, c.event_id, lead, None, note, confirmed_since, now, locale)
				.await?;
		}
		for p in payments {
			let note = Note::PaymentReceived {
				brand: p.brand_id,
				lead: p.lead_id,
				billed: p.billed,
				commission: p.commission,
				currency: p.currency,
			};
			queued += self
				.fan_out_one(&mut conn, Rule::PaymentReceived, p.event_id, None, None, |_| Ok(note.clone()), confirmed_since, now, locale)
				.await?;
		}
		for s in silent {
			// One message per full day of the same silence: at 24 h, at 48 h, …
			let days = now.duration_since(s.since).as_hours() / 24;
			let event = derived_event_id(&["source_silent", &s.key_id, &s.since.to_string(), &days.to_string()]);
			let note = Note::SourceSilent {
				key_id: s.key_id,
				kind: s.kind,
				last: s.last,
			};
			queued += self
				.fan_out_one(&mut conn, Rule::SourceSilent, event, None, None, |_| Ok(note.clone()), confirmed_since, now, locale)
				.await?;
		}
		Ok(queued)
	}

	/// One candidate: claimed, then queued for everyone who may hear of it but `skip` — in
	/// one transaction, so a claim never stands without its messages.
	#[expect(clippy::too_many_arguments, reason = "the candidate and the pass's context, each named at the call site")]
	async fn fan_out_one(
		&self,
		conn: &mut sqlx::PgConnection,
		rule: Rule,
		event: Uuid,
		lead: Option<(&str, &str)>,
		skip: Option<Uuid>,
		note: impl Fn(Role) -> eyre::Result<Note>,
		confirmed_since: Timestamp,
		now: Timestamp,
		locale: Locale,
	) -> eyre::Result<usize> {
		let mut tx = sqlx::Connection::begin(&mut *conn).await.wrap_err("beginning a fan-out")?;
		if !db::claim_fanout(&mut tx, rule, event, now).await? {
			return Ok(0);
		}
		let mut queued = 0;
		for r in db::recipients(&mut tx, rule, confirmed_since).await? {
			if !rule.open_to(r.role) || skip == Some(r.user_id) {
				continue;
			}
			let note = note(r.role)?;
			let text = Zeroizing::new(note.text(locale));
			let sealed = self.key.seal(&telegram_text_aad(rule.as_str(), event, r.chat_id), text.as_bytes())?;
			let message = NewMessage {
				user_id: r.user_id,
				chat_id: r.chat_id,
				rule,
				event_id: event,
				lead,
				buttons: note.buttons().iter().map(|b| b.as_str()).collect(),
				text_sealed: &sealed,
				data_key_fp: self.key.fingerprint(),
			};
			if db::enqueue(&mut tx, &message, now).await? {
				queued += 1;
			}
		}
		tx.commit().await.wrap_err("committing a fan-out")?;
		if queued > 0 {
			tracing::info!(%rule, %event, queued, "telegram: queued");
		}
		Ok(queued)
	}

	/// A lead as a message to `role` shows it: the PII of its creation only for a role that
	/// sees PII in the panel (§5.4).
	fn lead_note(&self, c: &LeadCandidate, role: Role) -> eyre::Result<LeadNote> {
		let mut note = LeadNote {
			brand: c.brand_id.clone(),
			location: c.location_id.clone(),
			..LeadNote::default()
		};
		if !role.sees_pii() {
			return Ok(note);
		}
		if let Some((blob, fp)) = &c.pii {
			eyre::ensure!(fp.as_slice() == self.key.fingerprint(), "the PII of event {} was sealed under another PANEL_DATA_KEY", c.event_id);
			let plain = self.key.open(&pii_aad(c.event_id), blob).wrap_err_with(|| format!("opening the PII of event {}", c.event_id))?;
			let pii: Value = serde_json::from_slice(&plain).wrap_err("stored PII is not JSON")?;
			note.name = text_field(&pii, "name");
			note.need = text_field(&pii, "need");
			note.phone = text_field(&pii, "phone");
		}
		Ok(note)
	}
}

/// The bot's side of the panel: what it sends and what it does with what it receives.
#[derive(Clone, Debug)]
pub struct Notifier<B, C> {
	pub panel: Panel,
	pub bot: B,
	/// concierge: to rotate a session's tokens and to ask `GetMe` with them.
	pub concierge: C,
	pub locale: Locale,
}

impl<B: Bot, C: Refresher + Directory> Notifier<B, C> {
	fn buttons(&self, chat_id: i64, message: i64, buttons: impl IntoIterator<Item = Button>) -> Vec<InlineButton> {
		let key = self.panel.key.telegram_callback_key();
		buttons
			.into_iter()
			.map(|b| InlineButton {
				label: b.label(self.locale),
				data: notify::callback_data(&key, chat_id, message, b),
			})
			.collect()
	}

	/// Sends what is due at `now`, within the pace (see `store::telegram::claim_due`).
	pub async fn deliver(&self, now: Timestamp) -> eyre::Result<Delivered> {
		let due = {
			let mut conn = self.panel.store.pool().acquire().await.wrap_err("a connection to claim messages")?;
			db::claim_due(&mut conn, now, now + SEND_LEASE).await?
		};
		futures::stream::iter(due)
			.map(|d| self.deliver_one(d, now))
			.buffer_unordered(PARALLEL_SENDS)
			.try_fold(Delivered::default(), |mut sum, one| async move {
				match one {
					Outcome::Sent => sum.sent += 1,
					Outcome::Retrying => sum.retrying += 1,
					Outcome::Dead => sum.dead += 1,
				}
				Ok(sum)
			})
			.await
	}

	async fn deliver_one(&self, d: db::Due, now: Timestamp) -> eyre::Result<Outcome> {
		let key = &self.panel.key;
		let mut conn = self.panel.store.pool().acquire().await.wrap_err("a connection to deliver")?;
		if d.data_key_fp.as_slice() != key.fingerprint() {
			db::dead(&mut conn, d.id, "sealed under another PANEL_DATA_KEY").await?;
			return Ok(Outcome::Dead);
		}
		let text = key
			.open(&telegram_text_aad(&d.rule, d.event_id, d.chat_id), &d.text_sealed)
			.wrap_err_with(|| format!("opening queued message {}", d.id))?;
		let text = String::from_utf8(text.to_vec()).wrap_err("a queued text is not UTF-8")?;
		let text = Zeroizing::new(text);
		let buttons = self.buttons(d.chat_id, d.id, d.buttons.iter().filter_map(|b| Button::from_name(b)));
		// No pool connection is held while Telegram is asked.
		drop(conn);
		let answer = self.bot.send(d.chat_id, &text, &buttons).await;
		let mut conn = self.panel.store.pool().acquire().await.wrap_err("a connection to deliver")?;
		let failure = match answer {
			Ok(message_id) => {
				db::sent(&mut conn, d.id, message_id, now).await?;
				return Ok(Outcome::Sent);
			}
			Err(f) => f,
		};
		let error = match &failure {
			Failure::RetryAfter(wait) => format!("429, retry after {}s", wait.as_secs()),
			Failure::Transient(why) | Failure::Refused(why) => why.clone(),
			Failure::Blocked => "403: the bot is blocked".to_owned(),
		};
		tracing::warn!(message = d.id, chat = d.chat_id, attempts = d.attempts, error, "telegram: a send failed");
		Ok(match notify::after_failure(d.attempts, &failure, now) {
			Next::Retry { at } => {
				db::retry(&mut conn, d.id, d.chat_id, at, &error, matches!(failure, Failure::RetryAfter(_))).await?;
				Outcome::Retrying
			}
			Next::Dead => {
				db::dead(&mut conn, d.id, &error).await?;
				Outcome::Dead
			}
			Next::ChatDead => {
				db::chat_dead(&mut conn, d.chat_id, now).await?;
				db::dead(&mut conn, d.id, &error).await?;
				tracing::info!(chat = d.chat_id, user_id = %d.user_id, "telegram: the bot is blocked; chat marked dead");
				Outcome::Dead
			}
		})
	}

	/// Confirms the access of linked users not confirmed for [`RECHECK_AFTER`], and prunes
	/// what is past keeping. How many were asked about.
	pub async fn recheck_access(&self, now: Timestamp) -> eyre::Result<usize> {
		let users = {
			let mut conn = self.panel.store.pool().acquire().await.wrap_err("a connection for access checks")?;
			db::prune(&mut conn, now - KEEP).await?;
			db::stale_access(&mut conn, now - RECHECK_AFTER, RECHECKS).await?
		};
		for user in &users {
			let access = self.confirm_access(*user, now).await?;
			tracing::debug!(user_id = %user, ?access, "telegram: access rechecked");
		}
		Ok(users.len())
	}

	/// Asks concierge, now, what the user may do — with their newest live panel session,
	/// rotated when due — and records the answer on their link.
	pub async fn confirm_access(&self, user: Uuid, now: Timestamp) -> eyre::Result<Access> {
		let panel = &self.panel;
		let session = match panel.session_of_user(user, now, &self.concierge).await {
			Ok(s) => Some(s),
			Err(SessionError::Missing | SessionError::Rejected) => None,
			Err(SessionError::Unavailable) => {
				self.tried(user, now).await?;
				return Ok(Access::Unavailable);
			}
			Err(SessionError::Internal(e)) => return Err(e),
		};
		let Some(session) = session else {
			self.tried(user, now).await?;
			return Ok(Access::NoSession);
		};
		let conn = || async { panel.store.pool().acquire().await.wrap_err("a connection to record access") };
		match self.concierge.me(&session.access).await {
			Ok(id) if id.user_id != user => Err(eyre::eyre!("GetMe answered another user than the session's")),
			Ok(Identity { role: Some(role), display_name, .. }) => {
				db::access_seen(&mut *conn().await?, user, Some(role), &display_name, now).await?;
				Ok(Access::Granted(role, display_name))
			}
			Ok(Identity { role: None, .. }) => {
				db::access_lost(&mut *conn().await?, user, now).await?;
				Ok(Access::Denied)
			}
			Err(DirectoryError::Refused) => {
				// The token is dead at concierge: so is the session, and whatever it vouched for.
				panel.close_session(&session.key).await?;
				db::access_lost(&mut *conn().await?, user, now).await?;
				Ok(Access::NoSession)
			}
			Err(DirectoryError::Unavailable(why)) => {
				tracing::warn!(why, "telegram: concierge unavailable for an access check");
				self.tried(user, now).await?;
				Ok(Access::Unavailable)
			}
			Err(e @ DirectoryError::Failed(_)) => Err(eyre::eyre!(e)),
		}
	}

	async fn tried(&self, user: Uuid, now: Timestamp) -> eyre::Result<()> {
		let mut conn = self.panel.store.pool().acquire().await.wrap_err("a connection to record access")?;
		db::access_tried(&mut conn, user, now).await
	}

	async fn reply(&self, chat: i64, reply: Reply) {
		if let Err(f) = self.bot.send(chat, reply.text(self.locale), &[]).await {
			tracing::warn!(chat, failure = ?f, "telegram: a reply was not delivered");
		}
	}

	/// Handles one update. Groups and channels are ignored whatever they say: the panel only
	/// ever talks to a person, one to one.
	pub async fn handle(&self, update: Update, now: Timestamp) -> eyre::Result<()> {
		match update {
			Update::Start { chat, .. } | Update::Stop { chat } | Update::Other { chat } if !chat.private => Ok(()),
			Update::Start { chat, payload: Some(token) } => {
				let mut conn = self.panel.store.pool().acquire().await.wrap_err("a connection to link")?;
				let reply = match db::redeem_link_token(&mut conn, &link_token_hash(token.trim()), now).await? {
					Some((user, role, display)) => {
						let mut tx = sqlx::Connection::begin(&mut *conn).await.wrap_err("beginning a link")?;
						db::link(&mut tx, user, chat.id, role, &display, now).await?;
						tx.commit().await.wrap_err("committing a link")?;
						tracing::info!(user_id = %user, "telegram: chat linked");
						Reply::Linked
					}
					None => Reply::LinkInvalid,
				};
				drop(conn);
				self.reply(chat.id, reply).await;
				Ok(())
			}
			Update::Start { chat, payload: None } | Update::Other { chat } => {
				self.reply(chat.id, Reply::Help).await;
				Ok(())
			}
			Update::Stop { chat } => {
				let mut conn = self.panel.store.pool().acquire().await.wrap_err("a connection to unlink")?;
				let gone = db::unlink_chat(&mut conn, chat.id).await?;
				drop(conn);
				self.reply(chat.id, if gone { Reply::Unlinked } else { Reply::NotLinked }).await;
				Ok(())
			}
			Update::Callback {
				id,
				chat_id,
				message_id,
				data,
				text,
			} => {
				let (reply, edit) = self.press(chat_id, &data, now).await?;
				if let Some((outbox, button, who)) = edit {
					let text = format!("{text}\n\n{}", notify::pressed_line(button, &who, self.locale));
					let buttons = self.buttons(chat_id, outbox, notify::after_press(button).iter().copied());
					if let Err(f) = self.bot.edit(chat_id, message_id, &text, &buttons).await {
						tracing::warn!(chat = chat_id, failure = ?f, "telegram: a pressed message was not updated");
					}
				}
				if let Err(f) = self.bot.answer(&id, reply.text(self.locale)).await {
					tracing::warn!(failure = ?f, "telegram: a button press was not answered");
				}
				Ok(())
			}
		}
	}

	/// A button pressed in `chat_id`: what to answer, and — when something was recorded now —
	/// the message, the button and who pressed it, for the message's new text. Nothing in the button is trusted
	/// but its signature: the message it names must be this chat's, the chat must be linked to
	/// the message's user, and that user's access is asked of concierge now.
	async fn press(&self, chat_id: i64, data: &str, now: Timestamp) -> eyre::Result<(Reply, Option<(i64, Button, String)>)> {
		let Some((outbox, button)) = notify::parse_callback(&self.panel.key.telegram_callback_key(), chat_id, data) else {
			tracing::warn!(chat = chat_id, "telegram: a button with data the panel did not sign");
			return Ok((Reply::BadButton, None));
		};
		let mut conn = self.panel.store.pool().acquire().await.wrap_err("a connection for a button")?;
		let message = db::message(&mut conn, outbox).await?;
		let link = db::link_of_chat(&mut conn, chat_id).await?;
		drop(conn);
		let Some(message) = message.filter(|m| m.chat_id == chat_id && m.buttons.iter().any(|b| b == button.as_str())) else {
			return Ok((Reply::BadButton, None));
		};
		let Some(link) = link.filter(|l| l.user_id == message.user_id && !l.dead) else {
			return Ok((Reply::NotLinked, None));
		};
		let (role, who) = match self.confirm_access(link.user_id, now).await? {
			Access::Granted(role, who) => (role, who),
			Access::NoSession => return Ok((Reply::OpenPanel, None)),
			Access::Denied => return Ok((Reply::NoAccess, None)),
			Access::Unavailable => return Ok((Reply::TryLater, None)),
		};
		if !role.edits_leads() {
			return Ok((Reply::NoAccess, None));
		}
		let (Some(brand), Some(lead)) = (message.brand_id.as_deref(), message.lead_id.as_deref()) else {
			return Ok((Reply::BadButton, None));
		};
		let (brand, lead) = (BrandId::parse(brand).wrap_err("a queued brand id")?, LeadId::parse(lead).wrap_err("a queued lead id")?);
		// The same button of the same message is one action, however often it is pressed or
		// its update delivered: the event ids derive from it.
		let key = format!("tg/{outbox}/{}", button.as_str());
		let by = Actor(link.user_id);
		let done = match button {
			Button::Take => self.panel.move_lead_once(by, &brand, &lead, StageMove::Contacted { channel: None }, now, Some(&key)).await,
			Button::NoAnswer => self.panel.no_answer_once(by, &brand, &lead, now, &key).await,
		};
		match done {
			Ok(done) if done.replayed => Ok((Reply::AlreadyDone, None)),
			Ok(_) => {
				tracing::info!(user_id = %link.user_id, button = button.as_str(), %brand, "telegram: a button recorded");
				Ok((Reply::Done, Some((outbox, button, who))))
			}
			Err(ActionError::NotFound) => Ok((Reply::LeadGone, None)),
			Err(ActionError::Conflict(_) | ActionError::Invalid(_)) => Ok((Reply::AlreadyDone, None)),
			Err(ActionError::Internal(e)) => Err(e),
		}
	}
}

enum Outcome {
	Sent,
	Retrying,
	Dead,
}

impl Panel {
	/// Takes or renews the poller's lease for `holder`: the update id to ask Telegram for
	/// next, or `None` while another replica polls.
	pub async fn telegram_poll_lease(&self, holder: Uuid, now: Timestamp, until: Timestamp) -> eyre::Result<Option<i64>> {
		let mut conn = self.store.pool().acquire().await.wrap_err("a connection for the poller")?;
		db::poll_lease(&mut conn, holder, now, until).await
	}

	/// Records that updates before `next_update_id` are handled; `false` when `holder` has
	/// lost the lease meanwhile.
	pub async fn telegram_poll_advance(&self, holder: Uuid, next_update_id: i64) -> eyre::Result<bool> {
		let mut conn = self.store.pool().acquire().await.wrap_err("a connection for the poller")?;
		db::poll_advance(&mut conn, holder, next_update_id).await
	}

	pub async fn telegram_poll_release(&self, holder: Uuid) -> eyre::Result<()> {
		let mut conn = self.store.pool().acquire().await.wrap_err("a connection for the poller")?;
		db::poll_release(&mut conn, holder).await
	}
}
