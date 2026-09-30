//! The Service-Arb panel without I/O: no database, network, clock or randomness. Time
//! comes in as an argument.
//!
//! - [`ids`]: the typed identities events carry;
//! - [`event`]: the envelope of a `sa.funnel.v1` event, and what a signing key may write;
//! - [`fact`]: the registered event types, typed and checked;

pub mod event;
pub mod fact;
pub mod ids;

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
