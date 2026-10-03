//! What the panel tells PostHog of a lead's life after the form (`sa_lead_created`,
//! `sa_lead_contacted`, …), so the funnel from a visit to a payment is one funnel there.
//!
//! Never PII: a brand, a location, the closed vocabularies (channel, flow, outcome, a loss's
//! reason slug) and amounts. Never the customer's words, a note, a name or a phone. The person
//! is the landing's analytics id from the lead's creation, so the visit and the lead are one
//! person in PostHog; a lead without one (typed in, or from a landing before the field) is
//! `sa-lead:<brand>:<lead>`.

use std::collections::BTreeMap;

use serde_json::{Value, json};

use crate::{
	fact::{AnalyticsId, Fact},
	lead::Recorded,
};

/// One event for PostHog: its name and properties. Its `uuid` is the journal's event id, so a
/// retry is deduplicated there.
#[derive(Clone, Debug, PartialEq)]
pub struct Capture {
	pub event: &'static str,
	pub properties: BTreeMap<&'static str, Value>,
}

/// What PostHog is told of a journaled event, if anything: `sa_` and its type, dots made
/// underscores. A call attempt is not (its outcome is), nor what is about no lead.
pub fn capture_of(e: &Recorded) -> Option<Capture> {
	let mut p: BTreeMap<&'static str, Value> = BTreeMap::new();
	let event = match &e.fact {
		Fact::LeadCreated { channel, suspect, offer, .. } => {
			p.insert("channel", json!(channel.as_str()));
			if let Some(flow) = offer.flow {
				p.insert("flow", json!(flow.as_str()));
			}
			if let Some(price) = offer.price {
				p.insert("quoted_cents", json!(price.cents));
			}
			if let Some(s) = suspect {
				p.insert("suspect", json!(s.as_str()));
			}
			"sa_lead_created"
		}
		Fact::LeadContacted { channel } => {
			if let Some(c) = channel {
				p.insert("channel", json!(c.as_str()));
			}
			"sa_lead_contacted"
		}
		Fact::LeadQuoted { quote } => {
			if let Some(q) = quote {
				p.insert("amount_cents", json!(q.minor));
				p.insert("currency", json!(q.currency.as_str()));
			}
			"sa_lead_quoted"
		}
		Fact::JobWon => "sa_job_won",
		// The reason is a slug the operator picks; the note is their words, and stays.
		Fact::LeadLost { reason, .. } => {
			p.insert("reason", json!(reason));
			"sa_lead_lost"
		}
		Fact::JobCompleted => "sa_job_completed",
		Fact::PaymentReceived { billed, commission } => {
			p.insert("billed_cents", json!(billed.minor));
			p.insert("commission_cents", json!(commission));
			p.insert("currency", json!(billed.currency.as_str()));
			"sa_payment_received"
		}
		Fact::CallLogged { outcome, .. } => {
			p.insert("outcome", json!(outcome.as_str()));
			"sa_call_logged"
		}
		Fact::CallAttempted | Fact::RetiredCount | Fact::ExperimentsDeclared(_) | Fact::ExperimentConfigured { .. } => return None,
	};
	e.subject.lead_id.as_ref()?;
	p.insert("brand_id", json!(e.subject.brand_id.as_str()));
	if let Some(l) = &e.subject.location_id {
		p.insert("location_id", json!(l.as_str()));
	}
	p.insert("manual", json!(e.source_kind.is_manual()));
	Some(Capture { event, properties: p })
}

/// Who the lead is in PostHog: the landing's analytics id, else one made of the lead.
pub fn distinct_id(brand: &str, lead: &str, analytics_id: Option<&AnalyticsId>) -> String {
	match analytics_id {
		Some(id) => id.as_str().to_owned(),
		None => format!("sa-lead:{brand}:{lead}"),
	}
}

#[cfg(test)]
mod tests {
	use jiff::Timestamp;
	use uuid::Uuid;

	use super::*;
	use crate::{
		event::{SourceKind, Subject},
		fact::{CallOutcome, LeadChannel, LeadOffer},
		ids::{BrandId, EventId, LeadId, LocationId},
	};

	fn rec(fact: Fact) -> Recorded {
		Recorded {
			id: EventId::from_raw(Uuid::now_v7()),
			occurred_at: Timestamp::UNIX_EPOCH,
			received_at: Timestamp::UNIX_EPOCH,
			source_kind: SourceKind::Panel,
			subject: Subject {
				brand_id: BrandId::parse("aquafix").unwrap(),
				location_id: Some(LocationId::parse("paris-11").unwrap()),
				lead_id: Some(LeadId::parse("L-1").unwrap()),
				job_id: None,
			},
			fact,
		}
	}

	#[test]
	fn names_and_no_pii() {
		let lost = capture_of(&rec(Fact::lost("too_expensive", Some("called him a liar, +33 6 00".into())).unwrap())).unwrap();
		assert_eq!(lost.event, "sa_lead_lost");
		assert_eq!(lost.properties["reason"], "too_expensive");
		assert!(!format!("{:?}", lost.properties).contains("+33"), "the note stays home");
		let paid = capture_of(&rec(Fact::payment(12_000, 1_800, "EUR").unwrap())).unwrap();
		assert_eq!((paid.event, paid.properties["billed_cents"].clone()), ("sa_payment_received", json!(12_000)));
		let created = capture_of(&rec(Fact::LeadCreated {
			channel: LeadChannel::Form,
			entered_by: Some("op-1".into()),
			suspect: None,
			offer: LeadOffer::parse(Some("fixed"), Some(9_900), Some("2026-10-01"), []).unwrap(),
			analytics_id: None,
		}))
		.unwrap();
		assert_eq!(created.event, "sa_lead_created");
		assert_eq!(
			created.properties.keys().copied().collect::<Vec<_>>(),
			["brand_id", "channel", "flow", "location_id", "manual", "quoted_cents"]
		);
		let call = capture_of(&rec(Fact::CallLogged {
			outcome: CallOutcome::NoAnswer,
			attempt_id: None,
		}))
		.unwrap();
		assert_eq!((call.event, call.properties["outcome"].clone()), ("sa_call_logged", json!("no_answer")));
		assert_eq!(capture_of(&rec(Fact::CallAttempted)), None);
		assert_eq!(capture_of(&rec(Fact::RetiredCount)), None);
	}

	#[test]
	fn the_person() {
		assert_eq!(distinct_id("aquafix", "L-1", None), "sa-lead:aquafix:L-1");
		let id = AnalyticsId::parse("0192f1c2-7d1e-7b3a-9c4d-1a2b3c4d5e6f").unwrap();
		assert_eq!(distinct_id("aquafix", "L-1", Some(&id)), id.as_str());
	}
}
