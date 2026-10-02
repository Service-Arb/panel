//! The Service-Arb panel without I/O: no database, network, clock or randomness. Time
//! comes in as an argument.
//!
//! - [`ids`]: the typed identities events carry;
//! - [`event`]: the envelope of a `sa.funnel.v1` event, and what a signing key may write;
//! - [`fact`]: the registered event types, typed and checked;
//! - [`lead`]: a lead's stage and stage times, folded from its facts;
//! - [`funnel`]: shares no more precise than the data, and the contact SLA;
//! - [`metrics`]: the aggregate stages and the experiments' daily counts, and how a newer
//!   count of a day replaces an older one;
//! - [`experiment`]: a variant against its control, no more sure than the data;
//! - [`signature`]: how a source signs a batch, and how the panel checks it;
//! - [`role`]: who may do what inside the panel;
//! - [`place`]: a place's live settings (phones, hours, …), checked as the sites read them;
//! - [`notify`]: the Telegram rules, their messages and buttons, and how a delivery is retried.

pub mod event;
pub mod experiment;
pub mod fact;
pub mod funnel;
pub mod ids;
pub mod lead;
pub mod metrics;
pub mod notify;
pub mod place;
pub mod role;
pub mod signature;

/// Why an event (or a part of one) cannot be accepted. The message is what the source is
/// told, so it names the field and never echoes a secret or PII.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[error("{0}")]
pub struct Invalid(pub String);

impl Invalid {
	pub fn new(msg: impl Into<String>) -> Self {
		Self(msg.into())
	}
}
