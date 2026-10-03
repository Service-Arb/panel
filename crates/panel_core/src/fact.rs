//! The registered event types, typed. A [`Fact`] is what an event says once its properties
//! have been checked against `type@type_version`; the checks that need the subject (a lead
//! event without a lead) are here too.

use std::fmt;

use crate::{Invalid, event::Subject, ids::is_slug, metrics::DailyMetric};

/// ISO 4217 code: three uppercase letters.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Currency(String);

impl Currency {
	pub fn parse(field: &str, raw: &str) -> Result<Self, Invalid> {
		if raw.len() == 3 && raw.bytes().all(|b| b.is_ascii_uppercase()) {
			Ok(Self(raw.to_owned()))
		} else {
			Err(Invalid::new(format!("{field} is not an ISO 4217 code like \"EUR\"")))
		}
	}

	pub fn as_str(&self) -> &str {
		&self.0
	}
}

impl fmt::Display for Currency {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.write_str(&self.0)
	}
}

/// An amount in minor units (cents) of its currency.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Money {
	pub minor: i64,
	pub currency: Currency,
}

/// How a lead came in.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LeadChannel {
	/// The landing's form.
	Form,
	/// A call that bypassed the form, entered by the operator (spec §10a).
	PhoneInbound,
	/// The landing's "call me back" request: the customer left a number to be called on.
	Callback,
}

impl LeadChannel {
	pub fn as_str(self) -> &'static str {
		match self {
			Self::Form => "form",
			Self::PhoneInbound => "phone_inbound",
			Self::Callback => "callback",
		}
	}

	pub fn parse(raw: &str) -> Result<Self, Invalid> {
		match raw {
			"form" => Ok(Self::Form),
			"phone_inbound" => Ok(Self::PhoneInbound),
			"callback" => Ok(Self::Callback),
			_ => Err(Invalid::new("properties.channel is not one of form, phone_inbound, callback")),
		}
	}
}

/// Why a landing's antispam doubted a lead it still sent: it may be a person, so it is kept
/// and shown, marked. A lead the honeypot caught never arrives, so it has no mark here.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LeadSuspect {
	/// The visitor's address sent more than the landing allows in its window.
	RateLimited,
	/// The form came back sooner after it was shown than a person types.
	TooFast,
}

impl LeadSuspect {
	pub fn as_str(self) -> &'static str {
		match self {
			Self::RateLimited => "rate_limited",
			Self::TooFast => "too_fast",
		}
	}

	pub fn parse(raw: &str) -> Result<Self, Invalid> {
		match raw {
			"rate_limited" => Ok(Self::RateLimited),
			"too_fast" => Ok(Self::TooFast),
			_ => Err(Invalid::new("properties.suspect is not one of rate_limited, too_fast")),
		}
	}
}

/// How the customer was reached.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContactChannel {
	Phone,
	Whatsapp,
	Email,
	Telegram,
	Other,
}

impl ContactChannel {
	pub fn as_str(self) -> &'static str {
		match self {
			Self::Phone => "phone",
			Self::Whatsapp => "whatsapp",
			Self::Email => "email",
			Self::Telegram => "telegram",
			Self::Other => "other",
		}
	}

	pub fn parse(raw: &str) -> Result<Self, Invalid> {
		match raw {
			"phone" => Ok(Self::Phone),
			"whatsapp" => Ok(Self::Whatsapp),
			"email" => Ok(Self::Email),
			"telegram" => Ok(Self::Telegram),
			"other" => Ok(Self::Other),
			_ => Err(Invalid::new("properties.channel is not one of phone, whatsapp, email, telegram, other")),
		}
	}
}

/// How a call ended, as the operator tells it. Only what a person can know without
/// telephony: no durations, no missed calls (spec §10.1).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CallOutcome {
	Answered,
	NoAnswer,
	WrongNumber,
	/// The customer asked to be called back.
	Later,
}

impl CallOutcome {
	pub fn as_str(self) -> &'static str {
		match self {
			Self::Answered => "answered",
			Self::NoAnswer => "no_answer",
			Self::WrongNumber => "wrong_number",
			Self::Later => "later",
		}
	}

	pub fn parse(raw: &str) -> Result<Self, Invalid> {
		match raw {
			"answered" => Ok(Self::Answered),
			"no_answer" => Ok(Self::NoAnswer),
			"wrong_number" => Ok(Self::WrongNumber),
			"later" => Ok(Self::Later),
			_ => Err(Invalid::new("properties.outcome is not one of answered, no_answer, wrong_number, later")),
		}
	}
}

/// What a registered event says.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Fact {
	LeadCreated {
		channel: LeadChannel,
		entered_by: Option<String>,
		/// Set when the landing's antispam doubted it; `None` for an ordinary lead.
		suspect: Option<LeadSuspect>,
	},
	LeadContacted {
		channel: Option<ContactChannel>,
	},
	LeadQuoted {
		quote: Option<Money>,
	},
	JobWon,
	/// `reason` is a lowercase slug, so the reports can group by it.
	LeadLost {
		reason: String,
		note: Option<String>,
	},
	JobCompleted,
	PaymentReceived {
		billed: Money,
		commission: i64,
	},
	CallAttempted,
	CallLogged {
		outcome: CallOutcome,
		attempt_id: Option<String>,
	},
	/// A day's count of an aggregate stage or an experiment's variant (`site.metrics`,
	/// `contact.metrics`, `experiment.metrics`): about no lead.
	Metric(DailyMetric),
}

/// Free text a person typed: bounded, so a source cannot park a document in the journal.
const MAX_TEXT: usize = 1000;

impl Fact {
	/// `lead.quoted`: both or neither of amount and currency.
	pub fn quote(amount: Option<i64>, currency: Option<&str>) -> Result<Option<Money>, Invalid> {
		match (amount, currency) {
			(None, None) => Ok(None),
			(Some(minor), Some(c)) => {
				if minor < 0 {
					return Err(Invalid::new("properties.amount is negative"));
				}
				Ok(Some(Money {
					minor,
					currency: Currency::parse("properties.currency", c)?,
				}))
			}
			_ => Err(Invalid::new("properties.amount and properties.currency go together")),
		}
	}

	/// `lead.lost`.
	pub fn lost(reason: &str, note: Option<String>) -> Result<Self, Invalid> {
		if !is_slug(reason) {
			return Err(Invalid::new("properties.reason is not a lowercase slug, e.g. \"too_expensive\""));
		}
		Ok(Self::LeadLost {
			reason: reason.to_owned(),
			note: bounded("properties.note", note)?,
		})
	}

	/// `payment.received`: `0 ≤ commission ≤ billed`.
	pub fn payment(billed: i64, commission: i64, currency: &str) -> Result<Self, Invalid> {
		if billed < 0 {
			return Err(Invalid::new("properties.billed is negative"));
		}
		if !(0..=billed).contains(&commission) {
			return Err(Invalid::new("properties.commission is not between 0 and billed"));
		}
		Ok(Self::PaymentReceived {
			billed: Money {
				minor: billed,
				currency: Currency::parse("properties.currency", currency)?,
			},
			commission,
		})
	}

	/// What the fact needs to know about its subject: every lead type is about a lead, and
	/// the job types about a job too; a count is about no one, and an experiment's about no
	/// location either.
	pub fn check_subject(&self, subject: &Subject) -> Result<(), Invalid> {
		if let Self::Metric(m) = self {
			if subject.lead_id.is_some() || subject.job_id.is_some() {
				return Err(Invalid::new("a count names no lead and no job"));
			}
			if !m.located() && subject.location_id.is_some() {
				return Err(Invalid::new("an experiment's count names no location"));
			}
			return Ok(());
		}
		if subject.lead_id.is_none() {
			return Err(Invalid::new("subject.lead_id is required for this type"));
		}
		let needs_job = match self {
			Self::JobWon | Self::JobCompleted => true,
			Self::LeadCreated { .. }
			| Self::LeadContacted { .. }
			| Self::LeadQuoted { .. }
			| Self::LeadLost { .. }
			| Self::PaymentReceived { .. }
			| Self::CallAttempted
			| Self::CallLogged { .. }
			| Self::Metric(_) => false,
		};
		if needs_job && subject.job_id.is_none() {
			return Err(Invalid::new("subject.job_id is required for this type"));
		}
		Ok(())
	}
}

/// Free text, trimmed; empty is none.
pub fn bounded(field: &str, text: Option<String>) -> Result<Option<String>, Invalid> {
	let Some(text) = text else { return Ok(None) };
	let text = text.trim();
	if text.chars().count() > MAX_TEXT {
		return Err(Invalid::new(format!("{field} is longer than {MAX_TEXT} characters")));
	}
	Ok((!text.is_empty()).then(|| text.to_owned()))
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::ids::{BrandId, JobId, LeadId};

	#[test]
	fn payments_and_quotes() {
		assert!(Fact::payment(12_000, 1_800, "EUR").is_ok());
		assert!(Fact::payment(12_000, 12_001, "EUR").is_err());
		assert!(Fact::payment(-1, 0, "EUR").is_err());
		assert!(Fact::payment(1, 0, "eur").is_err());
		assert_eq!(Fact::quote(None, None).unwrap(), None);
		assert!(Fact::quote(Some(1), None).is_err());
	}

	#[test]
	fn subjects() {
		let mut subject = Subject {
			brand_id: BrandId::parse("aquafix").unwrap(),
			location_id: None,
			lead_id: None,
			job_id: None,
		};
		assert!(Fact::CallAttempted.check_subject(&subject).is_err());
		subject.lead_id = Some(LeadId::parse("L-1").unwrap());
		assert!(Fact::CallAttempted.check_subject(&subject).is_ok());
		assert_eq!(Fact::JobWon.check_subject(&subject).unwrap_err().0, "subject.job_id is required for this type");
		subject.job_id = Some(JobId::parse("J-1").unwrap());
		assert!(Fact::JobWon.check_subject(&subject).is_ok());
	}

	#[test]
	fn lost_reasons_are_slugs() {
		assert!(Fact::lost("too_expensive", None).is_ok());
		assert!(Fact::lost("Too expensive", None).is_err());
		assert!(Fact::lost("x", Some("a".repeat(1001))).is_err());
		assert_eq!(Fact::lost("x", Some("  ".into())).unwrap(), Fact::LeadLost { reason: "x".into(), note: None });
	}
}
