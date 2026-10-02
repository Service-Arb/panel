//! The panel's engine: a signed batch in, each event checked, journaled and projected.
//!
//! ```text
//! batch ─ signature (source's key) ─┬─ event ─ envelope ─ key may write it? ─ registry ─┐
//!                                   ├─ event …                                          │
//!                                   └─ …        one transaction per event: journal ─────┴─ projections
//! ```
//!
//! [`Panel`] is the facade the server talks to; [`wire`] turns protojson into the domain of
//! `panel_core`; [`store`] is SQLite; [`seal`] encrypts what must not sit in the clear.
//! [`operator`] is what a signed-in user does and reads, as events through the same journal;
//! [`session`] is signing in through concierge and the sessions that follow; [`telegram`] the
//! bot's notifications and buttons; [`posthog`] the hourly import of the site's counts, and
//! [`counts`] what the screens read of them; [`place`] the places' live settings the sites
//! read and the panel edits.

pub mod counts;
pub mod operator;
pub mod place;
pub mod posthog;
pub mod seal;
pub mod session;
pub mod store;
pub mod telegram;
#[cfg(feature = "testing")]
pub mod testing;
pub mod wire;

use std::{collections::BTreeSet, sync::Arc};

use eyre::WrapErr;
use jiff::Timestamp;
use panel_core::{
	Invalid,
	event::{Envelope, KeyGrant, SourceKind},
	ids::{BrandId, LeadId},
	lead::Recorded,
	signature::{self, SignatureError},
};
use serde_json::Value;
use sha2::{Digest, Sha256};
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

/// The [`IngestError::Unauthorized`] of a delivery outside the replay window: the only one a
/// caller is told apart. It is checked before the key, and says only that the caller's clock
/// and ours disagree.
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
pub struct NewSource {
	pub key_id: String,
	pub secret: Zeroizing<String>,
}

impl std::fmt::Debug for NewSource {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("NewSource").field("key_id", &self.key_id).field("secret", &"<redacted>").finish()
	}
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

/// An id derived from its parts, the same for the same parts: what makes a retry of something
/// the panel writes itself land on the journal's id instead of a second event.
///
/// Shaped as a UUIDv7 (version and variant bits), which is what the journal keys by, but its
/// time field is hash, not time: the journal orders by `occurred_at`/`received_at`, the id
/// only breaks ties.
pub fn derived_id(namespace: &[u8], parts: &[&[u8]]) -> uuid::Uuid {
	let mut h = Sha256::new();
	h.update(namespace);
	for (i, part) in parts.iter().enumerate() {
		if i > 0 {
			h.update([0]);
		}
		h.update(part);
	}
	let digest = h.finalize();
	let mut bytes = [0u8; 16];
	bytes.copy_from_slice(&digest[..16]);
	uuid::Builder::from_bytes(bytes)
		.with_version(uuid::Version::SortRand)
		.with_variant(uuid::Variant::RFC4122)
		.into_uuid()
}

/// The engine.
#[derive(Clone, Debug)]
pub struct Panel {
	store: Store,
	key: Arc<DataKey>,
	rotations: Arc<session::Rotations>,
}

impl Panel {
	pub fn new(store: Store, key: DataKey) -> Self {
		Self {
			store,
			key: Arc::new(key),
			rotations: Arc::default(),
		}
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
		let refuse = |e: SignatureError| {
			IngestError::Unauthorized(match e {
				SignatureError::MalformedTimestamp | SignatureError::Stale => STALE,
				SignatureError::Mismatch => "bad signature",
			})
		};
		// Free checks first, before the database: the window, and whether the key id could
		// be one at all. A key id that could not is not logged — it is the caller's string.
		signature::check_window(batch.timestamp, now).map_err(refuse)?;
		if !panel_core::ids::is_slug(batch.key_id) {
			return Err(IngestError::Unauthorized("unknown key"));
		}
		let Some(source) = self.store.active_source(batch.key_id).await? else {
			// The MAC is computed anyway, against a key nobody holds, so an unknown key takes
			// as long to refuse as a bad signature and key ids cannot be told apart by timing.
			let _refused = signature::verify(&[0u8; 32], batch.timestamp, batch.signature, batch.body, now);
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
			refuse(e)
		})?;
		Ok(source.grant)
	}

	async fn one(&self, grant: &KeyGrant, raw: Value, now: Timestamp) -> eyre::Result<Outcome> {
		let incoming = match wire::decode(raw, now) {
			Ok(i) => i,
			Err(e) => return Ok(Outcome::Rejected(e)),
		};
		if let Err(e) = grant.permits(&incoming.envelope) {
			// Warned, not just answered: a key asking for more than it was given is either a
			// misconfigured source or a stolen key trying its luck.
			let env = &incoming.envelope;
			tracing::warn!(key_id = grant.key_id, brand = %env.subject.brand_id, kind = %env.source.kind, r#type = %env.type_key, reason = %e, "ingest: key not permitted");
			return Ok(Outcome::Rejected(e));
		}
		let checked = wire::check(&incoming.envelope.type_key, incoming.envelope.source.kind, &incoming.properties, &incoming.envelope.subject);
		let (status, _) = Status::of(&checked);
		let fact = match checked {
			Checked::Registered(fact) => Some(fact),
			Checked::Unregistered => None,
			Checked::Invalid(e) => return Ok(Outcome::Rejected(e)),
		};
		self.journal(&incoming, Some(&grant.key_id), status, fact, now).await
	}

	/// Journals an event the panel writes itself — an operator's action, an imported count —
	/// decoded and judged exactly as a source's, with no signing key. `Err` inside: the
	/// registry refuses it, and why.
	async fn write_own(&self, raw: Value, now: Timestamp) -> eyre::Result<Result<(Outcome, Envelope), Invalid>> {
		let incoming = match wire::decode(raw, now) {
			Ok(i) => i,
			Err(e) => return Ok(Err(e)),
		};
		let env = &incoming.envelope;
		let checked = wire::check(&env.type_key, env.source.kind, &incoming.properties, &env.subject);
		let (status, _) = Status::of(&checked);
		let fact = match checked {
			Checked::Registered(fact) => fact,
			Checked::Invalid(e) => return Ok(Err(e)),
			Checked::Unregistered => eyre::bail!("the panel wrote an unregistered type {}", env.type_key),
		};
		let outcome = self.journal(&incoming, None, status, Some(fact), now).await?;
		Ok(Ok((outcome, incoming.envelope)))
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
		// The write lock up front: a rebuild in progress is waited out, and so is another
		// event of the same lead (see `store`).
		let mut tx = self.store.begin_write().await?;
		let inserted = events::insert(
			&mut tx,
			&NewEvent {
				incoming,
				key_id,
				received_at: now,
				status,
				content_mac: self.key.content_mac(&incoming.canonical),
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
				received_at: now,
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
		// One write transaction from the first delete to the commit: ingest waits for it, and
		// readers see the old projections until then.
		let mut tx = self.store.begin_write().await?;
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
					received_at: stored.received_at,
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
