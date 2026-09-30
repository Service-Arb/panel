//! Telegram: link tokens, links, the users' rules, the fan-out marks, the outbox and the
//! poller's lease. Every timestamp is the caller's `now`, never the database's clock, so the
//! pacing can be driven by a test's virtual time.

use chrono::{DateTime, Utc};
use eyre::WrapErr;
use jiff::{SignedDuration, Timestamp};
use panel_core::{
	notify::{GLOBAL_PER_SECOND, PER_CHAT_GAP, Rule},
	role::Role,
};
use sqlx::PgConnection;
use uuid::Uuid;

use super::{from_pg, to_pg};

/// The advisory lock every claim of due messages takes: claims are short, and serializing
/// them is what keeps the global rate across replicas.
const PACE_LOCK: &str = "sa-panel/telegram/pace";

fn role_of(raw: Option<String>) -> eyre::Result<Option<Role>> {
	raw.map(|r| r.parse::<Role>().wrap_err("a stored role")).transpose()
}

// ── link tokens ─────────────────────────────────────────────────────────────────────────

/// Stores a link token's hash; expired ones are dropped on the way.
pub async fn insert_link_token(conn: &mut PgConnection, hash: &[u8], user: Uuid, role: Role, display: &str, now: Timestamp, expires_at: Timestamp) -> eyre::Result<()> {
	sqlx::query("DELETE FROM telegram_link_tokens WHERE expires_at <= $1")
		.bind(to_pg(now)?)
		.execute(&mut *conn)
		.await
		.wrap_err("pruning link tokens")?;
	sqlx::query("INSERT INTO telegram_link_tokens (token_hash, user_id, role, display_name, expires_at) VALUES ($1, $2, $3, $4, $5)")
		.bind(hash)
		.bind(user)
		.bind(role.as_str())
		.bind(display)
		.bind(to_pg(expires_at)?)
		.execute(&mut *conn)
		.await
		.wrap_err("storing a link token")?;
	Ok(())
}

/// Redeems a link token: whose it was, if it existed and had not expired. Gone either way,
/// so a token opens at most one link.
pub async fn redeem_link_token(conn: &mut PgConnection, hash: &[u8], now: Timestamp) -> eyre::Result<Option<(Uuid, Role, String)>> {
	let row: Option<(Uuid, String, String, DateTime<Utc>)> = sqlx::query_as("DELETE FROM telegram_link_tokens WHERE token_hash = $1 RETURNING user_id, role, display_name, expires_at")
		.bind(hash)
		.fetch_optional(&mut *conn)
		.await
		.wrap_err("redeeming a link token")?;
	let Some((user, role, display, expires_at)) = row else { return Ok(None) };
	if from_pg(expires_at)? <= now {
		return Ok(None);
	}
	Ok(Some((user, role.parse().wrap_err("a stored role")?, display)))
}

// ── links ───────────────────────────────────────────────────────────────────────────────

/// A user's chat with the bot.
#[derive(Clone, Debug)]
pub struct LinkRow {
	pub user_id: Uuid,
	pub chat_id: i64,
	pub role: Option<Role>,
	pub role_checked_at: Timestamp,
	pub display_name: String,
	pub dead: bool,
}

type LinkDb = (Uuid, i64, Option<String>, DateTime<Utc>, String, Option<DateTime<Utc>>);

fn link_row((user_id, chat_id, role, checked, display_name, dead_at): LinkDb) -> eyre::Result<LinkRow> {
	Ok(LinkRow {
		user_id,
		chat_id,
		role: role_of(role)?,
		role_checked_at: from_pg(checked)?,
		display_name,
		dead: dead_at.is_some(),
	})
}

// A macro, not a const, so every query stays a literal (`concat!`) that sqlx takes as audited.
/// Gives up what the outbox still owed: the filter follows.
macro_rules! drop_pending {
	() => {
		"UPDATE telegram_outbox SET state = 'dead', text_sealed = NULL, data_key_fp = NULL, leased_until = NULL, last_error = $1 WHERE state = 'pending' AND "
	};
}

/// Links `chat` to `user`: the user's previous chat and the chat's previous user are let go,
/// with what the outbox still owed them.
pub async fn link(conn: &mut PgConnection, user: Uuid, chat: i64, role: Role, display: &str, now: Timestamp) -> eyre::Result<()> {
	sqlx::query(concat!(drop_pending!(), "((user_id = $2 AND chat_id <> $3) OR (chat_id = $3 AND user_id <> $2))"))
		.bind("relinked")
		.bind(user)
		.bind(chat)
		.execute(&mut *conn)
		.await
		.wrap_err("dropping what a relinked chat was owed")?;
	sqlx::query("DELETE FROM telegram_links WHERE chat_id = $1 AND user_id <> $2")
		.bind(chat)
		.bind(user)
		.execute(&mut *conn)
		.await
		.wrap_err("moving a chat to another user")?;
	sqlx::query(
		"INSERT INTO telegram_links (user_id, chat_id, linked_at, role, role_checked_at, display_name) VALUES ($1, $2, $3, $4, $3, $5) \
		 ON CONFLICT (user_id) DO UPDATE SET chat_id = $2, linked_at = $3, role = $4, role_checked_at = $3, access_tried_at = NULL, display_name = $5, dead_at = NULL",
	)
	.bind(user)
	.bind(chat)
	.bind(to_pg(now)?)
	.bind(role.as_str())
	.bind(display)
	.execute(&mut *conn)
	.await
	.wrap_err("linking a chat")?;
	Ok(())
}

/// Unlinks a user's chat; `false` when there was none.
pub async fn unlink_user(conn: &mut PgConnection, user: Uuid) -> eyre::Result<bool> {
	sqlx::query(concat!(drop_pending!(), "user_id = $2"))
		.bind("unlinked")
		.bind(user)
		.execute(&mut *conn)
		.await
		.wrap_err("dropping what an unlinked user was owed")?;
	let gone = sqlx::query("DELETE FROM telegram_links WHERE user_id = $1")
		.bind(user)
		.execute(&mut *conn)
		.await
		.wrap_err("unlinking a user")?
		.rows_affected();
	Ok(gone == 1)
}

/// Unlinks a chat (`/stop`); `false` when it was not linked.
pub async fn unlink_chat(conn: &mut PgConnection, chat: i64) -> eyre::Result<bool> {
	sqlx::query(concat!(drop_pending!(), "chat_id = $2"))
		.bind("unlinked")
		.bind(chat)
		.execute(&mut *conn)
		.await
		.wrap_err("dropping what an unlinked chat was owed")?;
	let gone = sqlx::query("DELETE FROM telegram_links WHERE chat_id = $1")
		.bind(chat)
		.execute(&mut *conn)
		.await
		.wrap_err("unlinking a chat")?
		.rows_affected();
	Ok(gone == 1)
}

/// The bot was blocked in `chat`: nothing more goes there until the user links again.
pub async fn chat_dead(conn: &mut PgConnection, chat: i64, now: Timestamp) -> eyre::Result<()> {
	sqlx::query(concat!(drop_pending!(), "chat_id = $2"))
		.bind("the bot is blocked")
		.bind(chat)
		.execute(&mut *conn)
		.await
		.wrap_err("dropping what a dead chat was owed")?;
	sqlx::query("UPDATE telegram_links SET dead_at = $2 WHERE chat_id = $1 AND dead_at IS NULL")
		.bind(chat)
		.bind(to_pg(now)?)
		.execute(&mut *conn)
		.await
		.wrap_err("marking a chat dead")?;
	Ok(())
}

pub async fn link_of_user(conn: &mut PgConnection, user: Uuid) -> eyre::Result<Option<LinkRow>> {
	let row: Option<LinkDb> = sqlx::query_as("SELECT user_id, chat_id, role, role_checked_at, display_name, dead_at FROM telegram_links WHERE user_id = $1")
		.bind(user)
		.fetch_optional(&mut *conn)
		.await
		.wrap_err("reading a user's link")?;
	row.map(link_row).transpose()
}

pub async fn link_of_chat(conn: &mut PgConnection, chat: i64) -> eyre::Result<Option<LinkRow>> {
	let row: Option<LinkDb> = sqlx::query_as("SELECT user_id, chat_id, role, role_checked_at, display_name, dead_at FROM telegram_links WHERE chat_id = $1")
		.bind(chat)
		.fetch_optional(&mut *conn)
		.await
		.wrap_err("reading a chat's link")?;
	row.map(link_row).transpose()
}

/// What concierge just said of a linked user: their role (`None`: no access) and name.
pub async fn access_seen(conn: &mut PgConnection, user: Uuid, role: Option<Role>, display: &str, now: Timestamp) -> eyre::Result<()> {
	sqlx::query("UPDATE telegram_links SET role = $2, display_name = $3, role_checked_at = $4, access_tried_at = $4 WHERE user_id = $1")
		.bind(user)
		.bind(role.map(Role::as_str))
		.bind(display)
		.bind(to_pg(now)?)
		.execute(&mut *conn)
		.await
		.wrap_err("recording a user's access")?;
	Ok(())
}

/// A try to confirm a user's access that got no answer.
pub async fn access_tried(conn: &mut PgConnection, user: Uuid, now: Timestamp) -> eyre::Result<()> {
	sqlx::query("UPDATE telegram_links SET access_tried_at = $2 WHERE user_id = $1")
		.bind(user)
		.bind(to_pg(now)?)
		.execute(&mut *conn)
		.await
		.wrap_err("recording an access check")?;
	Ok(())
}

/// A user's access is known to be gone: nothing more is sent to them.
pub async fn access_lost(conn: &mut PgConnection, user: Uuid, now: Timestamp) -> eyre::Result<()> {
	sqlx::query("UPDATE telegram_links SET role = NULL, role_checked_at = $2, access_tried_at = $2 WHERE user_id = $1")
		.bind(user)
		.bind(to_pg(now)?)
		.execute(&mut *conn)
		.await
		.wrap_err("recording a user's lost access")?;
	Ok(())
}

/// Linked users whose access was last confirmed or tried before `before`, oldest first.
pub async fn stale_access(conn: &mut PgConnection, before: Timestamp, limit: i64) -> eyre::Result<Vec<Uuid>> {
	sqlx::query_scalar(
		"SELECT user_id FROM telegram_links WHERE dead_at IS NULL AND role_checked_at < $1 AND (access_tried_at IS NULL OR access_tried_at < $1) \
		 ORDER BY COALESCE(access_tried_at, role_checked_at) LIMIT $2",
	)
	.bind(to_pg(before)?)
	.bind(limit)
	.fetch_all(&mut *conn)
	.await
	.wrap_err("listing links to confirm")
}

// ── rules ───────────────────────────────────────────────────────────────────────────────

/// The rules a user chose; the others are at their default.
pub async fn rules(conn: &mut PgConnection, user: Uuid) -> eyre::Result<Vec<(Rule, bool)>> {
	let rows: Vec<(String, bool)> = sqlx::query_as("SELECT rule, enabled FROM telegram_rules WHERE user_id = $1")
		.bind(user)
		.fetch_all(&mut *conn)
		.await
		.wrap_err("reading a user's rules")?;
	rows.into_iter().map(|(r, on)| Ok((r.parse::<Rule>().wrap_err("a stored rule")?, on))).collect()
}

pub async fn set_rule(conn: &mut PgConnection, user: Uuid, rule: Rule, enabled: bool) -> eyre::Result<()> {
	sqlx::query("INSERT INTO telegram_rules (user_id, rule, enabled) VALUES ($1, $2, $3) ON CONFLICT (user_id, rule) DO UPDATE SET enabled = $3")
		.bind(user)
		.bind(rule.as_str())
		.bind(enabled)
		.execute(&mut *conn)
		.await
		.wrap_err("setting a rule")?;
	Ok(())
}

/// Who may get a rule's message now: linked, the chat alive, the rule on, and a role
/// concierge confirmed at or after `confirmed_since`. Whether the role is one the rule is
/// open to is the caller's to ask.
#[derive(Clone, Debug)]
pub struct Recipient {
	pub user_id: Uuid,
	pub chat_id: i64,
	pub role: Role,
}

pub async fn recipients(conn: &mut PgConnection, rule: Rule, confirmed_since: Timestamp) -> eyre::Result<Vec<Recipient>> {
	let rows: Vec<(Uuid, i64, String)> = sqlx::query_as(
		"SELECT l.user_id, l.chat_id, l.role FROM telegram_links l \
		 LEFT JOIN telegram_rules r ON r.user_id = l.user_id AND r.rule = $1 \
		 WHERE l.dead_at IS NULL AND l.role IS NOT NULL AND l.role_checked_at >= $2 AND COALESCE(r.enabled, $3) \
		 ORDER BY l.user_id",
	)
	.bind(rule.as_str())
	.bind(to_pg(confirmed_since)?)
	.bind(rule.on_by_default())
	.fetch_all(&mut *conn)
	.await
	.wrap_err("listing a rule's recipients")?;
	rows.into_iter()
		.map(|(user_id, chat_id, role)| {
			Ok(Recipient {
				user_id,
				chat_id,
				role: role.parse().wrap_err("a stored role")?,
			})
		})
		.collect()
}

// ── fan-out ─────────────────────────────────────────────────────────────────────────────

/// A lead's creation not yet fanned out for `rule`, with its sealed PII.
#[derive(Clone, Debug)]
pub struct LeadCandidate {
	pub event_id: Uuid,
	pub brand_id: String,
	pub lead_id: String,
	pub location_id: Option<String>,
	pub created_at: Option<Timestamp>,
	pub pii: Option<(Vec<u8>, Vec<u8>)>,
	/// The panel user who typed the lead in (its creation of kind `panel`), if one did.
	pub entered_by: Option<Uuid>,
}

type LeadCandidateDb = (Uuid, String, String, Option<String>, Option<DateTime<Utc>>, Option<Vec<u8>>, Option<Vec<u8>>, Option<String>);

fn lead_candidate((event_id, brand_id, lead_id, location_id, created_at, pii, fp, entered_by): LeadCandidateDb) -> eyre::Result<LeadCandidate> {
	Ok(LeadCandidate {
		event_id,
		brand_id,
		lead_id,
		location_id,
		created_at: created_at.map(from_pg).transpose()?,
		pii: pii.zip(fp),
		// A panel event's source id is the user's concierge id; anything else names no one.
		entered_by: entered_by.and_then(|id| Uuid::parse_str(&id).ok()),
	})
}

/// Leads whose creation (the one that counts: the first journaled) arrived at or after
/// `since`, still waiting for their first contact, and not yet told of.
pub async fn new_leads(conn: &mut PgConnection, since: Timestamp, limit: i64) -> eyre::Result<Vec<LeadCandidate>> {
	let rows: Vec<LeadCandidateDb> = sqlx::query_as(
		"SELECT e.id, l.brand_id, l.lead_id, l.location_id, l.created_at, e.pii_sealed, e.data_key_fp, \
		 CASE WHEN e.source_kind = 'panel' THEN e.source_id END \
		 FROM events e JOIN leads l ON l.brand_id = e.brand_id AND l.lead_id = e.lead_id \
		 WHERE e.type = 'lead.created' AND e.status = 'registered' AND e.received_at >= $1 AND l.stage = 'created' \
		 AND NOT EXISTS (SELECT 1 FROM events f WHERE f.brand_id = e.brand_id AND f.lead_id = e.lead_id AND f.type = 'lead.created' \
		                 AND f.status = 'registered' AND (f.received_at, f.id) < (e.received_at, e.id)) \
		 AND NOT EXISTS (SELECT 1 FROM telegram_fanout t WHERE t.rule = 'new_lead' AND t.event_id = e.id) \
		 ORDER BY e.received_at, e.id LIMIT $2",
	)
	.bind(to_pg(since)?)
	.bind(limit)
	.fetch_all(&mut *conn)
	.await
	.wrap_err("finding new leads to tell of")?;
	rows.into_iter().map(lead_candidate).collect()
}

/// Leads created in `[since, before)` and still not contacted — past the SLA when `before`
/// is now minus it — not yet reminded of. Keyed by their creation event.
pub async fn overdue_leads(conn: &mut PgConnection, since: Timestamp, before: Timestamp, limit: i64) -> eyre::Result<Vec<LeadCandidate>> {
	let rows: Vec<LeadCandidateDb> = sqlx::query_as(
		"SELECT c.id, l.brand_id, l.lead_id, l.location_id, l.created_at, c.pii_sealed, c.data_key_fp, \
		 CASE WHEN c.source_kind = 'panel' THEN c.source_id END FROM leads l \
		 JOIN LATERAL ( \
		   SELECT e.id, e.pii_sealed, e.data_key_fp, e.source_kind, e.source_id FROM events e \
		   WHERE e.brand_id = l.brand_id AND e.lead_id = l.lead_id AND e.type = 'lead.created' AND e.status = 'registered' \
		   ORDER BY e.received_at, e.id LIMIT 1 \
		 ) c ON true \
		 WHERE l.stage = 'created' AND l.contacted_at IS NULL AND l.created_at >= $1 AND l.created_at < $2 \
		 AND NOT EXISTS (SELECT 1 FROM telegram_fanout t WHERE t.rule = 'contact_overdue' AND t.event_id = c.id) \
		 ORDER BY l.created_at LIMIT $3",
	)
	.bind(to_pg(since)?)
	.bind(to_pg(before)?)
	.bind(limit)
	.fetch_all(&mut *conn)
	.await
	.wrap_err("finding overdue leads to remind of")?;
	rows.into_iter().map(lead_candidate).collect()
}

/// A payment not yet told of.
#[derive(Clone, Debug)]
pub struct PaymentCandidate {
	pub event_id: Uuid,
	pub brand_id: String,
	pub lead_id: String,
	pub billed: i64,
	pub commission: i64,
	pub currency: String,
}

/// Payments journaled at or after `since`, not yet told of.
pub async fn new_payments(conn: &mut PgConnection, since: Timestamp, limit: i64) -> eyre::Result<Vec<PaymentCandidate>> {
	let rows: Vec<(Uuid, String, String, i64, i64, String)> = sqlx::query_as(
		"SELECT p.event_id, p.brand_id, p.lead_id, p.billed, p.commission, p.currency FROM payments p JOIN events e ON e.id = p.event_id \
		 WHERE e.received_at >= $1 AND NOT EXISTS (SELECT 1 FROM telegram_fanout t WHERE t.rule = 'payment_received' AND t.event_id = p.event_id) \
		 ORDER BY e.received_at, e.id LIMIT $2",
	)
	.bind(to_pg(since)?)
	.bind(limit)
	.fetch_all(&mut *conn)
	.await
	.wrap_err("finding payments to tell of")?;
	Ok(rows
		.into_iter()
		.map(|(event_id, brand_id, lead_id, billed, commission, currency)| PaymentCandidate {
			event_id,
			brand_id,
			lead_id,
			billed,
			commission,
			currency,
		})
		.collect())
}

/// An active source that sent nothing since `before` (or never, and was added before it).
#[derive(Clone, Debug)]
pub struct SilentSource {
	pub key_id: String,
	pub kind: String,
	/// Its last event, if it ever sent one.
	pub last: Option<Timestamp>,
	/// Since when it is silent: its last event, or when it was added.
	pub since: Timestamp,
}

pub async fn silent_sources(conn: &mut PgConnection, before: Timestamp) -> eyre::Result<Vec<SilentSource>> {
	type Row = (String, String, Option<DateTime<Utc>>, DateTime<Utc>);
	let rows: Vec<Row> = sqlx::query_as(
		"SELECT s.key_id, s.kind, x.last, COALESCE(x.last, s.created_at) FROM sources s \
		 LEFT JOIN LATERAL (SELECT max(e.received_at) AS last FROM events e WHERE e.key_id = s.key_id) x ON true \
		 WHERE s.revoked_at IS NULL AND COALESCE(x.last, s.created_at) < $1 ORDER BY s.key_id",
	)
	.bind(to_pg(before)?)
	.fetch_all(&mut *conn)
	.await
	.wrap_err("finding silent sources")?;
	rows.into_iter()
		.map(|(key_id, kind, last, since)| {
			Ok(SilentSource {
				key_id,
				kind,
				last: last.map(from_pg).transpose()?,
				since: from_pg(since)?,
			})
		})
		.collect()
}

/// Marks `(rule, event_id)` as fanned out; `false` when it already was (by another replica).
pub async fn claim_fanout(conn: &mut PgConnection, rule: Rule, event_id: Uuid, now: Timestamp) -> eyre::Result<bool> {
	let done = sqlx::query("INSERT INTO telegram_fanout (rule, event_id, fanned_at) VALUES ($1, $2, $3) ON CONFLICT DO NOTHING")
		.bind(rule.as_str())
		.bind(event_id)
		.bind(to_pg(now)?)
		.execute(&mut *conn)
		.await
		.wrap_err("marking a fan-out")?
		.rows_affected();
	Ok(done == 1)
}

/// A message to queue.
pub struct NewMessage<'a> {
	pub user_id: Uuid,
	pub chat_id: i64,
	pub rule: Rule,
	pub event_id: Uuid,
	pub lead: Option<(&'a str, &'a str)>,
	pub buttons: Vec<&'static str>,
	pub text_sealed: &'a [u8],
	pub data_key_fp: [u8; 32],
}

/// Queues a message; `false` when `(rule, event, chat)` is queued already.
pub async fn enqueue(conn: &mut PgConnection, m: &NewMessage<'_>, now: Timestamp) -> eyre::Result<bool> {
	let done = sqlx::query(
		"INSERT INTO telegram_outbox (user_id, chat_id, rule, event_id, brand_id, lead_id, buttons, text_sealed, data_key_fp, next_attempt_at, created_at) \
		 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $10) ON CONFLICT (rule, event_id, chat_id) DO NOTHING",
	)
	.bind(m.user_id)
	.bind(m.chat_id)
	.bind(m.rule.as_str())
	.bind(m.event_id)
	.bind(m.lead.map(|(b, _)| b))
	.bind(m.lead.map(|(_, l)| l))
	.bind(&m.buttons)
	.bind(m.text_sealed)
	.bind(m.data_key_fp.as_slice())
	.bind(to_pg(now)?)
	.execute(&mut *conn)
	.await
	.wrap_err("queueing a message")?
	.rows_affected();
	Ok(done == 1)
}

/// Drops fan-out marks and finished messages older than `before`.
pub async fn prune(conn: &mut PgConnection, before: Timestamp) -> eyre::Result<()> {
	let before = to_pg(before)?;
	sqlx::query("DELETE FROM telegram_fanout WHERE fanned_at < $1")
		.bind(before)
		.execute(&mut *conn)
		.await
		.wrap_err("pruning fan-out marks")?;
	sqlx::query("DELETE FROM telegram_outbox WHERE state <> 'pending' AND created_at < $1")
		.bind(before)
		.execute(&mut *conn)
		.await
		.wrap_err("pruning the outbox")?;
	Ok(())
}

// ── delivery ────────────────────────────────────────────────────────────────────────────

/// A message claimed for sending.
#[derive(Clone, Debug)]
pub struct Due {
	pub id: i64,
	pub user_id: Uuid,
	pub chat_id: i64,
	pub rule: String,
	pub event_id: Uuid,
	pub buttons: Vec<String>,
	pub text_sealed: Vec<u8>,
	pub data_key_fp: Vec<u8>,
	/// Tries so far, this one included.
	pub attempts: i32,
}

/// Claims what may be sent at `now`, leased until `lease_until`: at most one message per chat
/// that has had none in the last [`PER_CHAT_GAP`] and none in flight, and no more than
/// [`GLOBAL_PER_SECOND`] tries started in the last second — across replicas, since every
/// claim holds one advisory lock. Oldest first.
pub async fn claim_due(conn: &mut PgConnection, now: Timestamp, lease_until: Timestamp) -> eyre::Result<Vec<Due>> {
	let mut tx = sqlx::Connection::begin(&mut *conn).await.wrap_err("beginning a claim")?;
	sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
		.bind(PACE_LOCK)
		.execute(&mut *tx)
		.await
		.wrap_err("taking the pacing lock")?;
	let now_pg = to_pg(now)?;
	let second_ago = to_pg(now - SignedDuration::from_secs(1))?;
	let gap_ago = to_pg(now - PER_CHAT_GAP)?;
	let started: i64 = sqlx::query_scalar("SELECT count(*) FROM telegram_outbox WHERE attempted_at > $1 AND attempted_at <= $2")
		.bind(second_ago)
		.bind(now_pg)
		.fetch_one(&mut *tx)
		.await
		.wrap_err("counting recent sends")?;
	let budget = (GLOBAL_PER_SECOND - started).max(0);
	if budget == 0 {
		tx.commit().await.wrap_err("ending a claim")?;
		return Ok(Vec::new());
	}
	type Row = (i64, Uuid, i64, String, Uuid, Vec<String>, Vec<u8>, Vec<u8>, i32);
	let rows: Vec<Row> = sqlx::query_as(
		"WITH busy AS ( \
		   SELECT DISTINCT chat_id FROM telegram_outbox \
		   WHERE (attempted_at > $2 AND attempted_at <= $1) OR (state = 'pending' AND leased_until > $1) \
		 ), pick AS ( \
		   SELECT DISTINCT ON (chat_id) id FROM telegram_outbox \
		   WHERE state = 'pending' AND next_attempt_at <= $1 AND (leased_until IS NULL OR leased_until <= $1) \
		     AND chat_id NOT IN (SELECT chat_id FROM busy) \
		   ORDER BY chat_id, id \
		 ), chosen AS (SELECT id FROM pick ORDER BY id LIMIT $4) \
		 UPDATE telegram_outbox o SET leased_until = $3, attempted_at = $1, attempts = o.attempts + 1 \
		 FROM chosen WHERE o.id = chosen.id \
		 RETURNING o.id, o.user_id, o.chat_id, o.rule, o.event_id, o.buttons, o.text_sealed, o.data_key_fp, o.attempts",
	)
	.bind(now_pg)
	.bind(gap_ago)
	.bind(to_pg(lease_until)?)
	.bind(budget)
	.fetch_all(&mut *tx)
	.await
	.wrap_err("claiming due messages")?;
	tx.commit().await.wrap_err("committing a claim")?;
	let mut due: Vec<Due> = rows
		.into_iter()
		.map(|(id, user_id, chat_id, rule, event_id, buttons, text_sealed, data_key_fp, attempts)| Due {
			id,
			user_id,
			chat_id,
			rule,
			event_id,
			buttons,
			text_sealed,
			data_key_fp,
			attempts,
		})
		.collect();
	due.sort_by_key(|d| d.id);
	Ok(due)
}

pub async fn sent(conn: &mut PgConnection, id: i64, tg_message_id: i64, now: Timestamp) -> eyre::Result<()> {
	sqlx::query(
		"UPDATE telegram_outbox SET state = 'sent', sent_at = $3, tg_message_id = $2, text_sealed = NULL, data_key_fp = NULL, leased_until = NULL, last_error = NULL \
		 WHERE id = $1",
	)
	.bind(id)
	.bind(tg_message_id)
	.bind(to_pg(now)?)
	.execute(&mut *conn)
	.await
	.wrap_err("recording a sent message")?;
	Ok(())
}

/// A failed try, tried again at `at`. With `whole_chat` (Telegram asked the chat to wait),
/// nothing else pending for the chat goes before `at` either.
pub async fn retry(conn: &mut PgConnection, id: i64, chat: i64, at: Timestamp, error: &str, whole_chat: bool) -> eyre::Result<()> {
	let at = to_pg(at)?;
	sqlx::query("UPDATE telegram_outbox SET next_attempt_at = $2, leased_until = NULL, last_error = $3 WHERE id = $1")
		.bind(id)
		.bind(at)
		.bind(error)
		.execute(&mut *conn)
		.await
		.wrap_err("scheduling a retry")?;
	if whole_chat {
		sqlx::query("UPDATE telegram_outbox SET next_attempt_at = GREATEST(next_attempt_at, $2) WHERE chat_id = $1 AND state = 'pending'")
			.bind(chat)
			.bind(at)
			.execute(&mut *conn)
			.await
			.wrap_err("holding a chat back")?;
	}
	Ok(())
}

/// Given up on.
pub async fn dead(conn: &mut PgConnection, id: i64, error: &str) -> eyre::Result<()> {
	sqlx::query("UPDATE telegram_outbox SET state = 'dead', text_sealed = NULL, data_key_fp = NULL, leased_until = NULL, last_error = $2 WHERE id = $1")
		.bind(id)
		.bind(error)
		.execute(&mut *conn)
		.await
		.wrap_err("giving up on a message")?;
	Ok(())
}

/// A sent message a button names.
#[derive(Clone, Debug)]
pub struct SentMessage {
	pub user_id: Uuid,
	pub chat_id: i64,
	pub brand_id: Option<String>,
	pub lead_id: Option<String>,
	pub buttons: Vec<String>,
}

pub async fn message(conn: &mut PgConnection, id: i64) -> eyre::Result<Option<SentMessage>> {
	type Row = (Uuid, i64, Option<String>, Option<String>, Vec<String>);
	let row: Option<Row> = sqlx::query_as("SELECT user_id, chat_id, brand_id, lead_id, buttons FROM telegram_outbox WHERE id = $1 AND state = 'sent'")
		.bind(id)
		.fetch_optional(&mut *conn)
		.await
		.wrap_err("reading a sent message")?;
	Ok(row.map(|(user_id, chat_id, brand_id, lead_id, buttons)| SentMessage {
		user_id,
		chat_id,
		brand_id,
		lead_id,
		buttons,
	}))
}

// ── the poller ──────────────────────────────────────────────────────────────────────────

/// Takes or renews the poller's lease for `holder` until `until`: the next update id to ask
/// for, or `None` when another replica holds it.
pub async fn poll_lease(conn: &mut PgConnection, holder: Uuid, now: Timestamp, until: Timestamp) -> eyre::Result<Option<i64>> {
	sqlx::query_scalar(
		"UPDATE telegram_poller SET holder = $1, leased_until = $3 \
		 WHERE holder = $1 OR holder IS NULL OR leased_until IS NULL OR leased_until <= $2 RETURNING next_update_id",
	)
	.bind(holder)
	.bind(to_pg(now)?)
	.bind(to_pg(until)?)
	.fetch_optional(&mut *conn)
	.await
	.wrap_err("leasing the poller")
}

/// Moves the offset past a handled update, if `holder` still holds the lease.
pub async fn poll_advance(conn: &mut PgConnection, holder: Uuid, next_update_id: i64) -> eyre::Result<bool> {
	let done = sqlx::query("UPDATE telegram_poller SET next_update_id = GREATEST(next_update_id, $2) WHERE holder = $1")
		.bind(holder)
		.bind(next_update_id)
		.execute(&mut *conn)
		.await
		.wrap_err("advancing the poller")?
		.rows_affected();
	Ok(done == 1)
}

/// Lets the lease go at shutdown, so another replica takes over at once.
pub async fn poll_release(conn: &mut PgConnection, holder: Uuid) -> eyre::Result<()> {
	sqlx::query("UPDATE telegram_poller SET holder = NULL, leased_until = NULL WHERE holder = $1")
		.bind(holder)
		.execute(&mut *conn)
		.await
		.wrap_err("releasing the poller")?;
	Ok(())
}
