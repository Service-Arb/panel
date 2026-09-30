//! The panel's engine: a signed batch in, each event checked, journaled and projected.
//!
//! ```text
//! batch ─ signature (source's key) ─┬─ event ─ envelope ─ key may write it? ─ registry ─┐
//!                                   ├─ event …                                          │
//!                                   └─ …        one transaction per event: journal ─────┴─ projections
//! ```
//!
//! [`Panel`] is the facade the server talks to; [`wire`] turns protojson into the domain of
//! `panel_core`; [`store`] is Postgres; [`seal`] encrypts what must not sit in the clear.

pub mod seal;
pub mod store;
#[cfg(feature = "testing")]
pub mod testing;
pub mod wire;

use std::{collections::BTreeSet, sync::Arc};

use eyre::WrapErr;
use jiff::Timestamp;
use panel_core::{
	Invalid,
	event::{KeyGrant, SourceKind},
	ids::{BrandId, LeadId},
	lead::Recorded,
	signature::{self, SignatureError},
};
use serde_json::Value;
use zeroize::Zeroizing;

use crate::{
	seal::{DataKey, pii_aad, source_secret_aad},
	store::{
		Store,
		events::{self, Inserted, NewEvent, Status},
		projections,
	},
	wire::{Checked, Incoming},
};

/// The [`IngestError::Unauthorized`] of a delivery outside the replay window: the only one
/// a caller may be told apart, since it is only ever reached under a valid signature.
pub const STALE: &str = "stale or malformed timestamp";

/// Why a whole batch was refused. Per-event verdicts are [`Outcome`]s instead.
#[derive(Debug, thiserror::Error)]
pub enum IngestError {
	/// No such key, a revoked one, a bad signature, or a delivery outside the window. The
	/// caller is told only that it was refused; the log says which.
	#[error("unauthorized: {0}")]
	Unauthorized(&'static str),
	/// The body as a whole is not a batch.
	#[error("{0}")]
	BadRequest(Invalid),
	#[error(transparent)]
	Internal(#[from] eyre::Report),
}

/// The verdict on one event of a batch.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Outcome {
	/// Journaled. With a type the registry does not know, it is not projected (yet).
	Accepted {
		unregistered: bool,
	},
	/// Already journaled.
	Duplicate,
	Rejected(Invalid),
}

/// One event's verdict, with where it was in the batch and the id it claimed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EventVerdict {
	pub index: usize,
	pub id: String,
	pub outcome: Outcome,
}

/// A signed request, as it arrived.
#[derive(Clone, Copy, Debug)]
pub struct SignedBatch<'a> {
	pub key_id: &'a str,
	pub timestamp: &'a str,
	pub signature: &'a str,
	pub body: &'a [u8],
}

/// A new source's credentials; the secret is shown once and never again.
#[derive(Debug)]
pub struct NewSource {
	pub key_id: String,
	pub secret: Zeroizing<String>,
}

/// What a rebuild did.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Rebuilt {
	pub events: u64,
	pub registered: u64,
	pub unregistered: u64,
	pub invalid: u64,
	pub leads: u64,
}

/// The engine.
#[derive(Clone, Debug)]
pub struct Panel {
	store: Store,
	key: Arc<DataKey>,
}

impl Panel {
	pub fn new(store: Store, key: DataKey) -> Self {
		Self { store, key: Arc::new(key) }
	}

	pub fn store(&self) -> &Store {
		&self.store
	}

	/// Registers a source that may write events of `kind` for `brands`, with a fresh
	/// secret. `None` when the key id is taken.
	pub async fn add_source(&self, key_id: &str, kind: SourceKind, brands: BTreeSet<BrandId>) -> eyre::Result<Option<NewSource>> {
		eyre::ensure!(panel_core::ids::is_slug(key_id), "a key id is a lowercase slug of 1–64 of [a-z0-9_-]");
		eyre::ensure!(!brands.is_empty(), "a source writes for at least one brand");
		let secret = Zeroizing::new(DataKey::generate_hex()?);
		let sealed = self.key.seal(&source_secret_aad(key_id), secret.as_bytes())?;
		let grant = KeyGrant {
			key_id: key_id.to_owned(),
			kind,
			brands,
		};
		let added = self.store.insert_source(&grant, &sealed, &self.key.fingerprint()).await?;
		Ok(added.then(|| NewSource { key_id: key_id.to_owned(), secret }))
	}

	/// Checks a batch's signature, then judges, journals and projects each of its events.
	pub async fn ingest(&self, batch: SignedBatch<'_>, now: Timestamp) -> Result<Vec<EventVerdict>, IngestError> {
		let grant = self.authenticate(&batch, now).await?;
		let raw = wire::batch(batch.body).map_err(IngestError::BadRequest)?;
		let mut verdicts = Vec::with_capacity(raw.len());
		for (index, event) in raw.into_iter().enumerate() {
			let id = wire::claimed_id(&event);
			let outcome = self.one(&grant, event, now).await?;
			verdicts.push(EventVerdict { index, id, outcome });
		}
		let count = |f: fn(&Outcome) -> bool| verdicts.iter().filter(|v| f(&v.outcome)).count();
		tracing::info!(
			key_id = grant.key_id,
			accepted = count(|o| matches!(o, Outcome::Accepted { .. })),
			duplicate = count(|o| matches!(o, Outcome::Duplicate)),
			rejected = count(|o| matches!(o, Outcome::Rejected(_))),
			"ingested a batch"
		);
		Ok(verdicts)
	}

	async fn authenticate(&self, batch: &SignedBatch<'_>, now: Timestamp) -> Result<KeyGrant, IngestError> {
		let Some(source) = self.store.active_source(batch.key_id).await? else {
			tracing::warn!(key_id = batch.key_id, "ingest: unknown or revoked key");
			return Err(IngestError::Unauthorized("unknown key"));
		};
		if source.data_key_fp != self.key.fingerprint() {
			return Err(IngestError::Internal(eyre::eyre!("source {} was sealed under another PANEL_DATA_KEY", batch.key_id)));
		}
		let secret = self
			.key
			.open(&source_secret_aad(batch.key_id), &source.secret_sealed)
			.wrap_err_with(|| format!("opening the secret of source {}", batch.key_id))?;
		signature::verify(&secret, batch.timestamp, batch.signature, batch.body, now).map_err(|e| {
			tracing::warn!(key_id = batch.key_id, error = %e, "ingest: signature refused");
			IngestError::Unauthorized(match e {
				SignatureError::MalformedTimestamp | SignatureError::Stale => STALE,
				SignatureError::Mismatch => "bad signature",
			})
		})?;
		Ok(source.grant)
	}

	async fn one(&self, grant: &KeyGrant, raw: Value, now: Timestamp) -> eyre::Result<Outcome> {
		let incoming = match wire::decode(raw, now) {
			Ok(i) => i,
			Err(e) => return Ok(Outcome::Rejected(e)),
		};
		if let Err(e) = grant.permits(&incoming.envelope) {
			return Ok(Outcome::Rejected(e));
		}
		let checked = wire::check(&incoming.envelope.type_key, &incoming.properties, &incoming.envelope.subject);
		let (status, _) = Status::of(&checked);
		let fact = match checked {
			Checked::Registered(fact) => Some(fact),
			Checked::Unregistered => None,
			Checked::Invalid(e) => return Ok(Outcome::Rejected(e)),
		};
		self.journal(&incoming, Some(&grant.key_id), status, fact, now).await
	}

	/// Journals one event and, when registered, projects it — in one transaction, so a
	/// projection never runs ahead of the journal nor behind it.
	async fn journal(&self, incoming: &Incoming, key_id: Option<&str>, status: Status, fact: Option<panel_core::fact::Fact>, now: Timestamp) -> eyre::Result<Outcome> {
		let pii = match &incoming.pii {
			Some(pii) => {
				let plain = Zeroizing::new(serde_json::to_vec(pii).wrap_err("serializing PII")?);
				Some((self.key.seal(&pii_aad(incoming.envelope.id.raw()), &plain)?, self.key.fingerprint()))
			}
			None => None,
		};
		let mut tx = self.store.pool().begin().await.wrap_err("beginning a transaction")?;
		let inserted = events::insert(
			&mut tx,
			&NewEvent {
				incoming,
				key_id,
				received_at: now,
				status,
				pii,
			},
		)
		.await?;
		match inserted {
			Inserted::New => {}
			// Nothing was written; dropping the transaction rolls back nothing.
			Inserted::Duplicate => return Ok(Outcome::Duplicate),
			Inserted::Conflict => return Ok(Outcome::Rejected(Invalid::new("id already names a different event"))),
		}
		if let Some(fact) = fact {
			let env = &incoming.envelope;
			let recorded = Recorded {
				id: env.id,
				occurred_at: env.occurred_at,
				source_kind: env.source.kind,
				subject: env.subject.clone(),
				fact,
			};
			projections::apply(&mut tx, &recorded).await?;
		}
		tx.commit().await.wrap_err("committing an event")?;
		Ok(Outcome::Accepted {
			unregistered: status == Status::Unregistered,
		})
	}

	/// An event's PII, opened. For the roles that may see it (spec §5.4); the caller checks.
	pub async fn pii(&self, id: uuid::Uuid) -> eyre::Result<Option<Value>> {
		/// `pii_sealed`, `data_key_fp`.
		type Sealed = (Option<Vec<u8>>, Option<Vec<u8>>);
		let row: Option<Sealed> = sqlx::query_as("SELECT pii_sealed, data_key_fp FROM events WHERE id = $1")
			.bind(id)
			.fetch_optional(self.store.pool())
			.await
			.wrap_err("reading an event's PII")?;
		let Some((Some(blob), Some(fp))) = row else { return Ok(None) };
		eyre::ensure!(fp == self.key.fingerprint(), "the PII of event {id} was sealed under another PANEL_DATA_KEY");
		let plain = self.key.open(&pii_aad(id), &blob).wrap_err_with(|| format!("opening the PII of event {id}"))?;
		Ok(Some(serde_json::from_slice(&plain).wrap_err("stored PII is not JSON")?))
	}

	/// Rebuilds the projections from the journal, in one transaction: every event is judged
	/// against the registry as it is now (its status updated to match), and the registered
	/// ones projected exactly as on arrival.
	pub async fn rebuild_projections(&self) -> eyre::Result<Rebuilt> {
		const PAGE: i64 = 1000;
		let mut tx = self.store.pool().begin().await.wrap_err("beginning the rebuild")?;
		projections::clear(&mut tx).await?;
		let mut done = Rebuilt::default();
		let mut leads: BTreeSet<(BrandId, LeadId)> = BTreeSet::new();
		let mut after = None;
		loop {
			let page = events::page(&mut tx, after, PAGE).await?;
			let Some(last) = page.last() else { break };
			after = Some((last.occurred_at, last.id));
			for stored in page {
				done.events += 1;
				let checked = stored.check();
				let (status, reason) = Status::of(&checked);
				if (status, reason.as_deref()) != (stored.status, stored.status_reason.as_deref()) {
					events::set_status(&mut tx, stored.id, status, reason.as_deref()).await?;
				}
				let fact = match checked {
					Checked::Registered(fact) => fact,
					Checked::Unregistered => {
						done.unregistered += 1;
						continue;
					}
					Checked::Invalid(_) => {
						done.invalid += 1;
						continue;
					}
				};
				done.registered += 1;
				let recorded = Recorded {
					id: stored.id,
					occurred_at: stored.occurred_at,
					source_kind: stored.source_kind,
					subject: stored.subject,
					fact,
				};
				projections::insert_row(&mut tx, &recorded).await?;
				if let Some(lead) = recorded.subject.lead_id {
					leads.insert((recorded.subject.brand_id, lead));
				}
			}
		}
		for (brand, lead) in &leads {
			projections::recompute_lead(&mut tx, brand, lead).await?;
		}
		done.leads = leads.len() as u64;
		tx.commit().await.wrap_err("committing the rebuild")?;
		Ok(done)
	}
}
