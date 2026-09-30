//! What an operator does in the panel (spec §5.4, §10a), as events.
//!
//! Every action becomes a `sa.funnel.v1` event of `source.kind = panel`, `source.id` the
//! user's concierge id, and goes through the same decode, registry and journal as a signed
//! batch — only with no signing key (`key_id` NULL). So the projections change the one way
//! they ever change, and a rebuild lands on the same state.
//!
//! And what the operator screens read: leads, a lead's card, the funnel.

use chrono::NaiveDate;
use eyre::WrapErr;
use jiff::{Timestamp, civil::Date};
use panel_contracts::SCHEMA;
use panel_core::{
	Invalid,
	fact::bounded,
	funnel::{self, Totals},
	ids::{BrandId, EventId, JobId, LeadId, LocationId},
};
use serde_json::{Map, Value, json};
use uuid::Uuid;

use crate::{
	Panel,
	seal::pii_aad,
	store::{
		events::Status,
		reads::{self, EventRow, LeadFilter, LeadRow, Sealed},
	},
	wire::{self, Checked},
};

/// Why an operator's action was not recorded.
#[derive(Debug, thiserror::Error)]
pub enum ActionError {
	/// No such lead, or no such call attempt of it.
	#[error("not found")]
	NotFound,
	/// The lead is not where the action needs it: completing a job never won.
	#[error("{0}")]
	Conflict(&'static str),
	/// What was entered does not pass the registry; the reason names the field.
	#[error("{0}")]
	Invalid(Invalid),
	#[error(transparent)]
	Internal(#[from] eyre::Report),
}

/// Who acts: the concierge user id of the signed-in operator or admin.
#[derive(Clone, Copy, Debug)]
pub struct Actor(pub Uuid);

/// A lead taken over the phone, bypassing the form (§10a "+ Call").
#[derive(Clone, Debug)]
pub struct NewLead {
	pub brand: BrandId,
	pub location: LocationId,
	/// What the customer needs, in their words: PII.
	pub need: String,
	pub phone: Option<String>,
}

/// A move of a lead through the funnel (stages 6–9).
#[derive(Clone, Debug)]
pub enum StageMove {
	Contacted {
		channel: Option<String>,
	},
	Quoted {
		amount: Option<i64>,
		currency: Option<String>,
	},
	/// A job is won; without an id, one is made.
	Won {
		job_id: Option<JobId>,
	},
	Lost {
		reason: String,
		note: Option<String>,
	},
	/// The lead's won job is done.
	Completed,
}

/// How an outgoing call ended.
#[derive(Clone, Debug)]
pub struct CallOutcome {
	pub attempt: Uuid,
	pub outcome: String,
}

/// A payment, entered by hand (owner, 2026-09-30).
#[derive(Clone, Debug)]
pub struct Payment {
	/// Minor units of `currency`.
	pub billed: i64,
	pub commission: i64,
	pub currency: String,
}

/// Whether the caller may see PII: the point where the role is asked (§5.4). Every role may
/// today; the parameter keeps the answer the caller's to give.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Pii {
	Reveal,
	Withhold,
}

/// A lead as the screens show it: the projection, and the PII of its creation when revealed.
#[derive(Clone, Debug)]
pub struct LeadView {
	pub row: LeadRow,
	pub pii: Option<Value>,
	pub waiting: Option<funnel::Waiting>,
}

/// One event on a lead's card.
#[derive(Clone, Debug)]
pub struct EventView {
	pub row: EventRow,
	pub pii: Option<Value>,
}

/// A page of leads; `next` continues after its last.
#[derive(Clone, Debug)]
pub struct LeadPage {
	pub leads: Vec<LeadView>,
	pub next: Option<(Timestamp, String, String)>,
}

/// Which leads to list, as the operator asks.
#[derive(Clone, Debug, Default)]
pub struct LeadQuery {
	pub stage: Option<panel_core::lead::Stage>,
	pub brand: Option<BrandId>,
	pub location: Option<LocationId>,
	/// Only those waiting for their first contact past [`funnel::CONTACT_SLA`].
	pub overdue: bool,
	pub after: Option<(Timestamp, String, String)>,
	pub limit: u32,
}

/// A page is at most this many leads.
pub const MAX_PAGE: u32 = 200;

fn opaque_new(prefix: &str, now: Timestamp) -> String {
	// A letter prefix keeps the id from ever being all digits, which `is_opaque` refuses as
	// looking like a phone number.
	format!("{prefix}-{}", new_uuid(now).simple())
}

fn new_uuid(now: Timestamp) -> Uuid {
	let nanos = u32::try_from(now.subsec_nanosecond().rem_euclid(1_000_000_000)).unwrap_or(0);
	let secs = u64::try_from(now.as_second()).unwrap_or(0);
	Uuid::new_v7(uuid::Timestamp::from_unix(uuid::NoContext, secs, nanos))
}

impl Panel {
	/// Records a lead that came in by phone: `lead.created{channel: phone_inbound}`, what it
	/// needs and the number in its PII. Its id is made here.
	pub async fn create_lead(&self, by: Actor, lead: NewLead, now: Timestamp) -> Result<(LeadId, EventId), ActionError> {
		let need = bounded("need", Some(lead.need))
			.map_err(ActionError::Invalid)?
			.ok_or_else(|| ActionError::Invalid(Invalid::new("need is required")))?;
		let phone = bounded("phone", lead.phone).map_err(ActionError::Invalid)?;
		let lead_id = LeadId::parse(&opaque_new("p", now)).map_err(|e| eyre::eyre!("a made lead id: {e}"))?;
		let mut pii = Map::new();
		pii.insert("need".into(), json!(need));
		if let Some(phone) = phone {
			pii.insert("phone".into(), json!(phone));
		}
		let subject = json!({"brandId": lead.brand.as_str(), "locationId": lead.location.as_str(), "leadId": lead_id.as_str()});
		let id = self
			.act(
				by,
				"lead.created",
				subject,
				json!({"channel": "phone_inbound", "enteredBy": by.0.to_string()}),
				Some(Value::Object(pii)),
				now,
			)
			.await?;
		Ok((lead_id, id))
	}

	/// Moves a lead: `lead.contacted`, `lead.quoted`, `job.won`, `lead.lost` or
	/// `job.completed`.
	pub async fn move_lead(&self, by: Actor, brand: &BrandId, lead: &LeadId, to: StageMove, now: Timestamp) -> Result<EventId, ActionError> {
		let current = self.lead_row(brand, lead).await?.ok_or(ActionError::NotFound)?;
		let mut subject = subject_of(&current);
		let (r#type, properties) = match to {
			StageMove::Contacted { channel } => ("lead.contacted", optional(&[("channel", channel.map(Value::from))])),
			StageMove::Quoted { amount, currency } => ("lead.quoted", optional(&[("amount", amount.map(Value::from)), ("currency", currency.map(Value::from))])),
			StageMove::Won { job_id } => {
				let job = match job_id {
					Some(job) => job,
					None => JobId::parse(&opaque_new("j", now)).map_err(|e| eyre::eyre!("a made job id: {e}"))?,
				};
				subject["jobId"] = json!(job.as_str());
				("job.won", json!({}))
			}
			StageMove::Lost { reason, note } => ("lead.lost", optional(&[("reason", Some(Value::from(reason))), ("note", note.map(Value::from))])),
			StageMove::Completed => {
				if current.job_id.is_none() {
					return Err(ActionError::Conflict("the lead has no won job to complete"));
				}
				("job.completed", json!({}))
			}
		};
		self.act(by, r#type, subject, properties, None, now).await
	}

	/// An outgoing call started from the panel (`call.attempted`); its event id is the attempt
	/// id the outcome names.
	pub async fn attempt_call(&self, by: Actor, brand: &BrandId, lead: &LeadId, now: Timestamp) -> Result<EventId, ActionError> {
		let current = self.lead_row(brand, lead).await?.ok_or(ActionError::NotFound)?;
		self.act(by, "call.attempted", subject_of(&current), json!({}), None, now).await
	}

	/// How an attempted call of this lead ended (`call.logged`).
	pub async fn log_call(&self, by: Actor, brand: &BrandId, lead: &LeadId, call: CallOutcome, now: Timestamp) -> Result<EventId, ActionError> {
		let current = self.lead_row(brand, lead).await?.ok_or(ActionError::NotFound)?;
		let mut conn = self.store.pool().acquire().await.wrap_err("a connection")?;
		if !reads::is_call_attempt(&mut conn, brand, lead, call.attempt).await? {
			return Err(ActionError::NotFound);
		}
		drop(conn);
		let properties = json!({"outcome": call.outcome, "attemptId": call.attempt.to_string()});
		self.act(by, "call.logged", subject_of(&current), properties, None, now).await
	}

	/// A payment for a lead (`payment.received`).
	pub async fn record_payment(&self, by: Actor, brand: &BrandId, lead: &LeadId, p: Payment, now: Timestamp) -> Result<EventId, ActionError> {
		let current = self.lead_row(brand, lead).await?.ok_or(ActionError::NotFound)?;
		let properties = json!({"billed": p.billed, "commission": p.commission, "currency": p.currency});
		self.act(by, "payment.received", subject_of(&current), properties, None, now).await
	}

	/// Builds the event and hands it to the journal exactly as ingest would.
	async fn act(&self, by: Actor, r#type: &str, subject: Value, properties: Value, pii: Option<Value>, now: Timestamp) -> Result<EventId, ActionError> {
		let mut raw = json!({
			"id": new_uuid(now).to_string(),
			"schema": SCHEMA,
			"type": r#type,
			"typeVersion": 1,
			"occurredAt": now.to_string(),
			"source": {"kind": "panel", "id": by.0.to_string()},
			"subject": subject,
			"properties": properties,
		});
		if let Some(pii) = pii {
			raw["pii"] = pii;
		}
		let incoming = wire::decode(raw, now).map_err(ActionError::Invalid)?;
		let env = &incoming.envelope;
		let checked = wire::check(&env.type_key, env.source.kind, &incoming.properties, &env.subject);
		let (status, _) = Status::of(&checked);
		let fact = match checked {
			Checked::Registered(fact) => fact,
			Checked::Invalid(e) => return Err(ActionError::Invalid(e)),
			Checked::Unregistered => return Err(ActionError::Internal(eyre::eyre!("the panel wrote an unregistered type {}", env.type_key))),
		};
		match self.journal(&incoming, None, status, Some(fact), now).await? {
			crate::Outcome::Accepted { .. } => {
				tracing::info!(user_id = %by.0, r#type, brand = %env.subject.brand_id, "operator action recorded");
				Ok(env.id)
			}
			crate::Outcome::Rejected(e) => Err(ActionError::Invalid(e)),
			crate::Outcome::Duplicate => Err(ActionError::Internal(eyre::eyre!("a fresh event id was taken"))),
		}
	}

	async fn lead_row(&self, brand: &BrandId, lead: &LeadId) -> eyre::Result<Option<LeadRow>> {
		let mut conn = self.store.pool().acquire().await.wrap_err("a connection")?;
		reads::lead(&mut conn, brand, lead).await
	}

	/// A page of leads, newest first.
	pub async fn leads(&self, q: &LeadQuery, pii: Pii, now: Timestamp) -> eyre::Result<LeadPage> {
		let limit = q.limit.clamp(1, MAX_PAGE);
		let filter = LeadFilter {
			stage: q.stage,
			brand: q.brand.clone(),
			location: q.location.clone(),
			waiting_since_before: q.overdue.then(|| now - funnel::CONTACT_SLA),
			after: q.after.clone(),
			limit: i64::from(limit) + 1,
		};
		let mut conn = self.store.pool().acquire().await.wrap_err("a connection")?;
		let mut rows = reads::leads(&mut conn, &filter).await?;
		drop(conn);
		let more = rows.len() > limit as usize;
		rows.truncate(limit as usize);
		let next = more.then(|| rows.last().map(|r| (r.sort_at, r.brand_id.clone(), r.lead_id.clone()))).flatten();
		let leads = rows.into_iter().map(|row| self.lead_view(row, pii, now)).collect::<eyre::Result<_>>()?;
		Ok(LeadPage { leads, next })
	}

	/// A lead and every event about it; `None` when there is no such lead.
	pub async fn lead_card(&self, brand: &BrandId, lead: &LeadId, pii: Pii, now: Timestamp) -> eyre::Result<Option<(LeadView, Vec<EventView>)>> {
		let mut conn = self.store.pool().acquire().await.wrap_err("a connection")?;
		let Some(row) = reads::lead(&mut conn, brand, lead).await? else { return Ok(None) };
		let events = reads::lead_events(&mut conn, brand, lead).await?;
		drop(conn);
		let events = events
			.into_iter()
			.map(|row| {
				let pii = match pii {
					Pii::Reveal => self.open_pii(&row.sealed)?,
					Pii::Withhold => None,
				};
				Ok(EventView { row, pii })
			})
			.collect::<eyre::Result<_>>()?;
		Ok(Some((self.lead_view(row, pii, now)?, events)))
	}

	/// The personal funnel over the leads that came in from `from` to `to` (UTC days, both
	/// included).
	pub async fn funnel(&self, from: Date, to: Date, brand: Option<&BrandId>) -> eyre::Result<Totals> {
		let day = |d: Date| NaiveDate::from_ymd_opt(i32::from(d.year()), u32::from(d.month().unsigned_abs()), u32::from(d.day().unsigned_abs())).ok_or_else(|| eyre::eyre!("date {d}"));
		let mut conn = self.store.pool().acquire().await.wrap_err("a connection")?;
		reads::funnel(&mut conn, day(from)?, day(to)?, brand).await
	}

	fn lead_view(&self, row: LeadRow, pii: Pii, now: Timestamp) -> eyre::Result<LeadView> {
		let revealed = match (pii, &row.creation) {
			(Pii::Reveal, Some(sealed)) => self.open_pii(sealed)?,
			_ => None,
		};
		Ok(LeadView {
			waiting: funnel::waiting(row.stage, row.created_at, row.contacted_at, now),
			pii: revealed,
			row,
		})
	}

	fn open_pii(&self, sealed: &Sealed) -> eyre::Result<Option<Value>> {
		let Some((blob, fp)) = &sealed.pii else { return Ok(None) };
		let id = sealed.event_id;
		eyre::ensure!(fp.as_slice() == self.key.fingerprint(), "the PII of event {id} was sealed under another PANEL_DATA_KEY");
		let plain = self.key.open(&pii_aad(id), blob).wrap_err_with(|| format!("opening the PII of event {id}"))?;
		Ok(Some(serde_json::from_slice(&plain).wrap_err("stored PII is not JSON")?))
	}
}

/// The subject of an action on a lead: its brand and id, and its location and job as the
/// projection has them.
fn subject_of(lead: &LeadRow) -> Value {
	let mut subject = json!({"brandId": lead.brand_id, "leadId": lead.lead_id});
	if let Some(location) = &lead.location_id {
		subject["locationId"] = json!(location);
	}
	if let Some(job) = &lead.job_id {
		subject["jobId"] = json!(job);
	}
	subject
}

/// An object of the fields that are set.
fn optional(fields: &[(&str, Option<Value>)]) -> Value {
	Value::Object(fields.iter().filter_map(|(k, v)| v.clone().map(|v| ((*k).to_owned(), v))).collect())
}
