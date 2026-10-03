//! The registered event types, typed. A [`Fact`] is what an event says once its properties
//! have been checked against `type@type_version`; the checks that need the subject (a lead
//! event without a lead) are here too.

use std::{collections::BTreeMap, fmt};

use jiff::civil::Date;

use crate::{
	Invalid,
	event::Subject,
	experiment::{Declaration, Patch},
	ids::is_slug,
};

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

/// Which of a landing's flows a lead came through (FORM-VARIANTS-SPEC): a quote asked for, a
/// price estimated from what the visitor picked, or a fixed price for a well-defined job.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LeadFlow {
	Quote,
	Estimate,
	Fixed,
}

impl LeadFlow {
	pub const ALL: [Self; 3] = [Self::Quote, Self::Estimate, Self::Fixed];

	pub fn as_str(self) -> &'static str {
		match self {
			Self::Quote => "quote",
			Self::Estimate => "estimate",
			Self::Fixed => "fixed",
		}
	}

	pub fn parse(raw: &str) -> Result<Self, Invalid> {
		Self::ALL
			.into_iter()
			.find(|f| f.as_str() == raw)
			.ok_or_else(|| Invalid::new("properties.flow is not one of quote, estimate, fixed"))
	}

	/// Whether the landing showed the visitor a price in this flow, and so must say which.
	pub fn is_priced(self) -> bool {
		match self {
			Self::Estimate | Self::Fixed => true,
			Self::Quote => false,
		}
	}
}

/// The price an estimate or a fixed-price flow showed: integer cents, EUR TTC (there is no
/// currency field yet), and the day the pricing model it came from took effect.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct QuotedPrice {
	pub cents: i64,
	pub valid_from: Date,
}

/// The most estimate inputs one lead may carry.
pub const MAX_ESTIMATE_INPUTS: usize = 12;

/// What a lead says of its flow and price. The default is a lead that says nothing of either:
/// what every landing sent before the flows.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct LeadOffer {
	pub flow: Option<LeadFlow>,
	/// Set exactly when the flow is priced.
	pub price: Option<QuotedPrice>,
	/// What the visitor picked for an estimate, input id → value id; empty for any other flow.
	/// Slugs, never free text: no PII.
	pub estimate_inputs: BTreeMap<String, String>,
}

impl LeadOffer {
	/// `lead.created`'s `flow`, `quoted_cents`, `pricing_valid_from` and `estimate_inputs`:
	/// the price both or neither, and exactly when the flow is priced; inputs with an
	/// estimate only.
	pub fn parse(flow: Option<&str>, quoted_cents: Option<i64>, valid_from: Option<&str>, inputs: impl IntoIterator<Item = (String, String)>) -> Result<Self, Invalid> {
		let flow = flow.map(LeadFlow::parse).transpose()?;
		let price = match (quoted_cents, valid_from) {
			(None, None) => None,
			(Some(cents), Some(day)) => {
				if cents < 0 {
					return Err(Invalid::new("properties.quoted_cents is negative"));
				}
				Some(QuotedPrice {
					cents,
					valid_from: rfc3339_date(day).ok_or_else(|| Invalid::new("properties.pricing_valid_from is not a date like \"2026-10-01\""))?,
				})
			}
			_ => return Err(Invalid::new("properties.quoted_cents and properties.pricing_valid_from go together")),
		};
		match (flow.is_some_and(LeadFlow::is_priced), price.is_some()) {
			(true, false) => return Err(Invalid::new("properties.quoted_cents and properties.pricing_valid_from are required with flow estimate or fixed")),
			(false, true) => return Err(Invalid::new("properties.quoted_cents and properties.pricing_valid_from are only for flow estimate or fixed")),
			_ => {}
		}
		let estimate_inputs: BTreeMap<String, String> = inputs.into_iter().collect();
		if !estimate_inputs.is_empty() && flow != Some(LeadFlow::Estimate) {
			return Err(Invalid::new("properties.estimate_inputs is only for flow estimate"));
		}
		if estimate_inputs.len() > MAX_ESTIMATE_INPUTS {
			return Err(Invalid::new(format!("properties.estimate_inputs holds more than {MAX_ESTIMATE_INPUTS} inputs")));
		}
		// The words, never the value: it may be anything a source put there.
		if !estimate_inputs.iter().all(|(k, v)| is_input_slug(k) && is_input_slug(v)) {
			return Err(Invalid::new("properties.estimate_inputs keys and values are 1–40 of [a-z0-9_-]"));
		}
		Ok(Self { flow, price, estimate_inputs })
	}
}

/// 1–40 of `[a-z0-9_-]`: an estimate's input or value id, as kitstart's pricing model names it.
fn is_input_slug(s: &str) -> bool {
	(1..=40).contains(&s.len()) && s.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'-')
}

/// `YYYY-MM-DD`, a real day: RFC 3339's full-date, nothing looser.
fn rfc3339_date(raw: &str) -> Option<Date> {
	let b = raw.as_bytes();
	let shaped = b.len() == 10 && b[4] == b'-' && b[7] == b'-' && b.iter().enumerate().all(|(i, c)| i == 4 || i == 7 || c.is_ascii_digit());
	shaped.then(|| raw.parse().ok()).flatten()
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

/// The `distinct_id` a landing's analytics beacon gave the visitor who sent a form: 1–128 of
/// `[A-Za-z0-9._:-]` (a UUID from `crypto.randomUUID()`, or the beacon's fallback). A random
/// id, not PII; PostHog knows the visit by it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AnalyticsId(String);

impl AnalyticsId {
	pub fn parse(raw: &str) -> Result<Self, Invalid> {
		let ok = (1..=128).contains(&raw.len()) && raw.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b':' | b'-'));
		if ok {
			Ok(Self(raw.to_owned()))
		} else {
			Err(Invalid::new("properties.analytics_id is not 1–128 of [A-Za-z0-9._:-]"))
		}
	}

	pub fn as_str(&self) -> &str {
		&self.0
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
		/// Its flow and the price it was shown; empty from a landing before the flows.
		offer: LeadOffer,
		/// The landing's analytics `distinct_id` when the form was sent ([`AnalyticsId`]).
		analytics_id: Option<AnalyticsId>,
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
	/// A day's count from the retired PostHog import (`site.metrics`, `contact.metrics`,
	/// `experiment.metrics`): still read, so a journal from before keeps passing the registry
	/// and a rebuild does not fail on it, but nothing is projected from it. PostHog itself is
	/// where those counts are looked at now.
	RetiredCount,
	/// What a brand's landing declares of its experiments at start: about no lead.
	ExperimentsDeclared(Vec<Declaration>),
	/// An admin's change to one experiment of the brand; `by` names them as the screens show it
	/// ([`crate::experiment::label`]).
	ExperimentConfigured {
		patch: Patch,
		by: String,
	},
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
	/// the job types about a job too; a count is about no one, an experiment about its brand.
	pub fn check_subject(&self, subject: &Subject) -> Result<(), Invalid> {
		/// What a type needs of its subject.
		enum Needs {
			Lead,
			LeadAndJob,
			NoLeadNoJob,
			BrandOnly,
		}
		let needs = match self {
			Self::JobWon | Self::JobCompleted => Needs::LeadAndJob,
			Self::LeadCreated { .. }
			| Self::LeadContacted { .. }
			| Self::LeadQuoted { .. }
			| Self::LeadLost { .. }
			| Self::PaymentReceived { .. }
			| Self::CallAttempted
			| Self::CallLogged { .. } => Needs::Lead,
			Self::RetiredCount => Needs::NoLeadNoJob,
			Self::ExperimentsDeclared(_) | Self::ExperimentConfigured { .. } => Needs::BrandOnly,
		};
		match needs {
			Needs::NoLeadNoJob if subject.lead_id.is_some() || subject.job_id.is_some() => Err(Invalid::new("a count names no lead and no job")),
			Needs::BrandOnly if subject.location_id.is_some() || subject.lead_id.is_some() || subject.job_id.is_some() =>
				Err(Invalid::new("an experiment is the brand's: subject names no location, lead or job")),
			Needs::Lead | Needs::LeadAndJob if subject.lead_id.is_none() => Err(Invalid::new("subject.lead_id is required for this type")),
			Needs::LeadAndJob if subject.job_id.is_none() => Err(Invalid::new("subject.job_id is required for this type")),
			Needs::Lead | Needs::LeadAndJob | Needs::NoLeadNoJob | Needs::BrandOnly => Ok(()),
		}
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
	fn a_lead_offer_is_priced_exactly_when_its_flow_is() {
		let none = std::iter::empty::<(String, String)>;
		let inputs = |n: usize| (0..n).map(|i| (format!("in-{i}"), "v_1".to_owned())).collect::<Vec<_>>();
		assert_eq!(LeadOffer::parse(None, None, None, none()).unwrap(), LeadOffer::default());
		assert_eq!(LeadOffer::parse(Some("quote"), None, None, none()).unwrap().flow, Some(LeadFlow::Quote));
		let estimate = LeadOffer::parse(Some("estimate"), Some(12_900), Some("2026-10-01"), inputs(12)).unwrap();
		assert_eq!(
			estimate.price,
			Some(QuotedPrice {
				cents: 12_900,
				valid_from: "2026-10-01".parse().unwrap()
			})
		);
		assert_eq!(estimate.estimate_inputs.len(), 12);
		assert!(LeadOffer::parse(Some("fixed"), Some(0), Some("2026-10-01"), none()).is_ok(), "a free job is a price");
		for (flow, cents, day, n, want) in [
			(Some("quote"), Some(100), Some("2026-10-01"), 0, "are only for flow estimate or fixed"),
			(None, Some(100), Some("2026-10-01"), 0, "are only for flow estimate or fixed"),
			(Some("estimate"), Some(100), None, 0, "go together"),
			(Some("estimate"), None, None, 0, "are required with flow estimate or fixed"),
			(Some("fixed"), None, None, 0, "are required with flow estimate or fixed"),
			(Some("fixed"), Some(100), Some("2026-10-01"), 1, "only for flow estimate"),
			(Some("quote"), None, None, 1, "only for flow estimate"),
			(Some("estimate"), Some(100), Some("2026-10-01"), 13, "more than 12"),
			(Some("estimate"), Some(-1), Some("2026-10-01"), 0, "negative"),
			(Some("estimate"), Some(100), Some("2026-10-01T00:00:00Z"), 0, "not a date"),
			(Some("estimate"), Some(100), Some("2026-02-30"), 0, "not a date"),
			(Some("Estimate"), None, None, 0, "not one of quote, estimate, fixed"),
		] {
			let e = LeadOffer::parse(flow, cents, day, inputs(n)).unwrap_err();
			assert!(e.0.contains(want), "{flow:?} {cents:?} {day:?} {n}: {e}");
		}
		for (k, v) in [("Zone", "a"), ("zone", ""), ("zone", "a b"), ("zone", &"a".repeat(41) as &str), ("zone", "+33600000000")] {
			let e = LeadOffer::parse(Some("estimate"), Some(1), Some("2026-10-01"), [(k.to_owned(), v.to_owned())]).unwrap_err();
			assert_eq!(e.0, "properties.estimate_inputs keys and values are 1–40 of [a-z0-9_-]", "{k:?} {v:?}");
		}
	}

	#[test]
	fn analytics_ids() {
		for good in ["0192f1c2-7d1e-7b3a-9c4d-1a2b3c4d5e6f", "18f3a2b-9c1d2e3f", "a", &"x".repeat(128) as &str, "A.b_c:d-1"] {
			assert_eq!(AnalyticsId::parse(good).unwrap().as_str(), good);
		}
		for bad in ["", &"x".repeat(129) as &str, "a b", "a/b", "é", "+33 6 00"] {
			assert!(AnalyticsId::parse(bad).is_err(), "{bad:?}");
		}
	}

	#[test]
	fn lost_reasons_are_slugs() {
		assert!(Fact::lost("too_expensive", None).is_ok());
		assert!(Fact::lost("Too expensive", None).is_err());
		assert!(Fact::lost("x", Some("a".repeat(1001))).is_err());
		assert_eq!(Fact::lost("x", Some("  ".into())).unwrap(), Fact::LeadLost { reason: "x".into(), note: None });
	}
}
