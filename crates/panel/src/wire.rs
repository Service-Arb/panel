//! protojson → the domain: one event of a batch, decoded and checked, and the registry of
//! the types the panel projects.
//!
//! Each event of a batch is decoded on its own, so one malformed event is rejected alone
//! rather than failing the batch.

use jiff::Timestamp;
use panel_contracts::{SCHEMA, v1};
use panel_core::{
	Invalid,
	event::{Envelope, Source, SourceKind, Subject, TypeKey},
	fact::{CallOutcome, ContactChannel, Fact, LeadChannel, bounded},
	ids::{BrandId, JobId, LeadId, LocationId, parse_event_id},
};
use serde::de::DeserializeOwned;
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

/// Events in one request, at most.
pub const MAX_BATCH: usize = 500;

/// An event that passed the envelope checks; its properties are not judged yet.
#[derive(Clone, Debug)]
pub struct Incoming {
	pub envelope: Envelope,
	/// A JSON object, as sent.
	pub properties: Value,
	/// A non-empty JSON object, or none.
	pub pii: Option<Value>,
	/// SHA-256 of the event in canonical form: the same event sent twice, in either field
	/// spelling or key order, hashes the same.
	pub content_sha256: [u8; 32],
}

/// The events of a request body: `{"events": [...]}`, 1 to [`MAX_BATCH`] of them. What is
/// wrong with the body as a whole is the caller's mistake.
pub fn batch(body: &[u8]) -> Result<Vec<Value>, Invalid> {
	let value: Value = serde_json::from_slice(body).map_err(|e| Invalid::new(format!("body is not JSON: {e}")))?;
	let Value::Object(mut top) = value else {
		return Err(Invalid::new("body is not a JSON object"));
	};
	if let Some(other) = top.keys().find(|k| *k != "events") {
		return Err(Invalid::new(format!("unknown field {other:?} in the body")));
	}
	let Some(Value::Array(events)) = top.remove("events") else {
		return Err(Invalid::new("body has no events array"));
	};
	if events.is_empty() || events.len() > MAX_BATCH {
		return Err(Invalid::new(format!("a batch holds 1 to {MAX_BATCH} events, not {}", events.len())));
	}
	Ok(events)
}

/// The id an event was sent with, if it has a string one: for the answer about it, even
/// when nothing else about it can be read.
pub fn claimed_id(raw: &Value) -> String {
	raw.get("id").and_then(Value::as_str).unwrap_or_default().to_owned()
}

/// Decodes one event and checks its envelope. `now` bounds how far in the future it may
/// have happened.
pub fn decode(raw: Value, now: Timestamp) -> Result<Incoming, Invalid> {
	// `properties` and `pii` are kept as sent: a protobuf `Struct` holds every number as a
	// double, and going through it would turn `12000` into `12000.0`, which an int64 field
	// then refuses. The typed decode below still checks they are objects.
	let object = |field: &str| match raw.get(field) {
		None | Some(Value::Null) => Value::Object(Map::new()),
		Some(v) => v.clone(),
	};
	let (properties, pii) = (object("properties"), object("pii"));
	let event: v1::Event = serde_json::from_value(raw).map_err(|e| Invalid::new(format!("not a {SCHEMA} event: {e}")))?;
	if event.schema != SCHEMA {
		return Err(Invalid::new(format!("schema is {:?}, not {SCHEMA:?}", event.schema)));
	}
	let content_sha256 = content_hash(&event)?;
	let id = parse_event_id(&event.id)?;
	let type_key = TypeKey::parse(&event.r#type, event.type_version)?;
	let occurred_at = event.occurred_at.as_ref().ok_or_else(|| Invalid::new("occurred_at is required"))?;
	let occurred_at = Timestamp::new(occurred_at.seconds, occurred_at.nanos)
		.map_err(|_| Invalid::new("occurred_at is out of range"))?
		// Postgres keeps microseconds; cut here so what is checked is what is stored.
		.round(jiff::TimestampRound::new().smallest(jiff::Unit::Microsecond).mode(jiff::RoundMode::Trunc))
		.map_err(|_| Invalid::new("occurred_at is out of range"))?;

	let source = event.source.as_ref().ok_or_else(|| Invalid::new("source is required"))?;
	if !(1..=128).contains(&source.id.chars().count()) {
		return Err(Invalid::new("source.id must be 1 to 128 characters"));
	}
	let source = Source {
		kind: source.kind.parse::<SourceKind>()?,
		id: source.id.clone(),
	};

	let subject = event.subject.as_ref().ok_or_else(|| Invalid::new("subject is required"))?;
	let subject = Subject {
		brand_id: BrandId::parse(&subject.brand_id)?,
		location_id: subject.location_id.as_deref().map(LocationId::parse).transpose()?,
		lead_id: subject.lead_id.as_deref().map(LeadId::parse).transpose()?,
		job_id: subject.job_id.as_deref().map(JobId::parse).transpose()?,
	};

	let envelope = Envelope {
		id,
		type_key,
		occurred_at,
		source,
		subject,
	};
	envelope.check_time(now)?;
	let pii = pii.as_object().is_some_and(|o| !o.is_empty()).then_some(pii);
	Ok(Incoming {
		envelope,
		properties,
		pii,
		content_sha256,
	})
}

fn content_hash(event: &v1::Event) -> Result<[u8; 32], Invalid> {
	let value = serde_json::to_value(event).map_err(|e| Invalid::new(format!("event does not serialize: {e}")))?;
	let mut h = Sha256::new();
	write_canonical(&value, &mut h);
	Ok(h.finalize().into())
}

/// JSON with object keys sorted at every level, whatever map the JSON library was built
/// with (a protobuf `Struct` is a hash map, so its order is not stable across runs).
fn write_canonical(value: &Value, h: &mut Sha256) {
	match value {
		Value::Object(map) => {
			let mut keys: Vec<&String> = map.keys().collect();
			keys.sort();
			h.update(b"{");
			for (i, k) in keys.into_iter().enumerate() {
				if i > 0 {
					h.update(b",");
				}
				h.update(Value::String(k.clone()).to_string().as_bytes());
				h.update(b":");
				write_canonical(&map[k], h);
			}
			h.update(b"}");
		}
		Value::Array(items) => {
			h.update(b"[");
			for (i, item) in items.iter().enumerate() {
				if i > 0 {
					h.update(b",");
				}
				write_canonical(item, h);
			}
			h.update(b"]");
		}
		scalar => h.update(scalar.to_string().as_bytes()),
	}
}

/// What the registry makes of an event's type and properties.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Checked {
	/// A registered type, and its properties and subject hold.
	Registered(Fact),
	/// Not a registered `type@version`: stored, not projected.
	Unregistered,
	/// A registered type whose checks the event fails.
	Invalid(Invalid),
}

/// Every `type@version` the panel projects, with the message its properties must match.
pub const REGISTERED: &[(&str, u32, &str)] = &[
	("lead.created", 1, "sa.v1.LeadCreatedV1"),
	("lead.contacted", 1, "sa.v1.LeadContactedV1"),
	("lead.quoted", 1, "sa.v1.LeadQuotedV1"),
	("job.won", 1, "sa.v1.JobWonV1"),
	("lead.lost", 1, "sa.v1.LeadLostV1"),
	("job.completed", 1, "sa.v1.JobCompletedV1"),
	("payment.received", 1, "sa.v1.PaymentReceivedV1"),
	("call.attempted", 1, "sa.v1.CallAttemptedV1"),
	("call.logged", 1, "sa.v1.CallLoggedV1"),
];

/// Checks an event's properties against its `type@version`, and what the type needs of its
/// subject. The same call judges an event on arrival and again when the projections are
/// rebuilt, so a type registered later picks up what arrived before it.
pub fn check(key: &TypeKey, properties: &Value, subject: &Subject) -> Checked {
	let fact = match (key.name.as_str(), key.version) {
		("lead.created", 1) => props::<v1::LeadCreatedV1>(key, properties).and_then(|p| {
			Ok(Fact::LeadCreated {
				channel: LeadChannel::parse(&p.channel)?,
				entered_by: bounded("properties.entered_by", p.entered_by)?,
			})
		}),
		("lead.contacted", 1) => props::<v1::LeadContactedV1>(key, properties).and_then(|p| {
			Ok(Fact::LeadContacted {
				channel: p.channel.as_deref().map(ContactChannel::parse).transpose()?,
			})
		}),
		("lead.quoted", 1) => props::<v1::LeadQuotedV1>(key, properties).and_then(|p| {
			Ok(Fact::LeadQuoted {
				quote: Fact::quote(p.amount, p.currency.as_deref())?,
			})
		}),
		("job.won", 1) => props::<v1::JobWonV1>(key, properties).map(|_| Fact::JobWon),
		("lead.lost", 1) => props::<v1::LeadLostV1>(key, properties).and_then(|p| Fact::lost(&p.reason, p.note)),
		("job.completed", 1) => props::<v1::JobCompletedV1>(key, properties).map(|_| Fact::JobCompleted),
		("payment.received", 1) => props::<v1::PaymentReceivedV1>(key, properties).and_then(|p| Fact::payment(p.billed, p.commission, &p.currency)),
		("call.attempted", 1) => props::<v1::CallAttemptedV1>(key, properties).map(|_| Fact::CallAttempted),
		("call.logged", 1) => props::<v1::CallLoggedV1>(key, properties).and_then(|p| {
			Ok(Fact::CallLogged {
				outcome: CallOutcome::parse(&p.outcome)?,
				attempt_id: bounded("properties.attempt_id", p.attempt_id)?,
			})
		}),
		_ => return Checked::Unregistered,
	};
	match fact.and_then(|f| f.check_subject(subject).map(|()| f)) {
		Ok(f) => Checked::Registered(f),
		Err(e) => Checked::Invalid(e),
	}
}

fn props<T: DeserializeOwned>(key: &TypeKey, properties: &Value) -> Result<T, Invalid> {
	serde_json::from_value(properties.clone()).map_err(|e| Invalid::new(format!("properties do not match {key}: {e}")))
}

#[cfg(test)]
mod tests {
	use serde_json::json;

	use super::*;

	fn now() -> Timestamp {
		"2026-09-30T12:00:00Z".parse().unwrap()
	}

	fn event() -> Value {
		json!({
			"id": uuid::Uuid::now_v7().to_string(),
			"schema": "sa.funnel.v1",
			"type": "lead.created",
			"typeVersion": 1,
			"occurredAt": "2026-09-30T10:00:00.123456789Z",
			"source": {"kind": "site", "id": "aquafix"},
			"subject": {"brandId": "aquafix", "locationId": "paris-11", "leadId": "L-1"},
			"properties": {"channel": "form"},
			"pii": {"phone": "+33 6 00 00 00 00", "nested": {"b": 1, "a": [1, 2]}}
		})
	}

	#[test]
	fn a_good_event() {
		let mut e = event();
		e["properties"]["n"] = json!(12000);
		let got = decode(e, now()).unwrap();
		assert_eq!(got.properties["n"], json!(12000), "an integer stays one, not a Struct's double");
		let got = decode(event(), now()).unwrap();
		assert_eq!(got.envelope.type_key.to_string(), "lead.created@1");
		assert_eq!(got.envelope.occurred_at.to_string(), "2026-09-30T10:00:00.123456Z", "cut to microseconds");
		assert_eq!(got.pii.unwrap()["phone"], "+33 6 00 00 00 00");
		assert_eq!(
			check(&got.envelope.type_key, &got.properties, &got.envelope.subject),
			Checked::Registered(Fact::LeadCreated {
				channel: LeadChannel::Form,
				entered_by: None
			})
		);
	}

	#[test]
	fn the_hash_ignores_spelling_and_key_order() {
		let camel = event();
		let mut snake = json!({});
		for (k, v) in camel.as_object().unwrap().iter().rev() {
			let k = match k.as_str() {
				"typeVersion" => "type_version",
				"occurredAt" => "occurred_at",
				other => other,
			};
			snake[k] = v.clone();
		}
		assert_eq!(decode(camel.clone(), now()).unwrap().content_sha256, decode(snake, now()).unwrap().content_sha256);
		let mut other = camel.clone();
		other["pii"]["phone"] = json!("+33 6 11 11 11 11");
		assert_ne!(decode(camel, now()).unwrap().content_sha256, decode(other, now()).unwrap().content_sha256);
	}

	#[test]
	fn envelope_rejections() {
		let cases: [(&str, Value, &str); 7] = [
			("schema", json!("sa.funnel.v2"), "schema is \"sa.funnel.v2\""),
			("id", json!("6ba7b810-9dad-11d1-80b4-00c04fd430c8"), "UUIDv1"),
			("occurredAt", json!("2026-09-30T12:06:00Z"), "in the future"),
			("source", json!({"kind": "email", "id": "x"}), "source.kind \"email\""),
			("subject", json!({"brandId": "Aquafix"}), "subject.brand_id"),
			("type", json!("Lead Created"), "dotted lowercase"),
			("extra", json!(1), "unknown field"),
		];
		for (field, value, want) in cases {
			let mut e = event();
			e[field] = value;
			let err = decode(e, now()).unwrap_err().0;
			assert!(err.contains(want), "{field}: {err}");
		}
	}

	#[test]
	fn the_registry() {
		let subject = decode(event(), now()).unwrap().envelope.subject;
		for (name, version, _) in REGISTERED {
			let got = check(&TypeKey::parse(name, *version).unwrap(), &json!({}), &subject);
			assert_ne!(got, Checked::Unregistered, "{name}@{version} is listed but not handled");
		}
		assert_eq!(check(&TypeKey::parse("lead.created", 2).unwrap(), &json!({"x": 1}), &subject), Checked::Unregistered);
		assert_eq!(check(&TypeKey::parse("review.new", 1).unwrap(), &json!({}), &subject), Checked::Unregistered);
		let Checked::Invalid(e) = check(&TypeKey::parse("call.logged", 1).unwrap(), &json!({"outcome": "voicemail"}), &subject) else {
			panic!("an unknown outcome passed")
		};
		assert!(e.0.contains("voicemail"), "{e}");
		let Checked::Invalid(e) = check(
			&TypeKey::parse("payment.received", 1).unwrap(),
			&json!({"billed": 100, "commission": 10, "currency": "EUR", "tip": 5}),
			&subject,
		) else {
			panic!("an unknown field passed")
		};
		assert!(e.0.contains("tip"), "{e}");
		assert_eq!(
			check(
				&TypeKey::parse("payment.received", 1).unwrap(),
				&json!({"billed": "12000", "commission": 1800, "currency": "EUR"}),
				&subject
			),
			Checked::Registered(Fact::payment(12_000, 1_800, "EUR").unwrap())
		);
	}

	#[test]
	fn batches() {
		assert!(batch(br#"{"events":[{}]}"#).is_ok());
		for (body, want) in [
			(&b"[]"[..], "not a JSON object"),
			(b"{", "not JSON"),
			(br#"{"events":[]}"#, "1 to 500"),
			(br#"{"events":[], "more": 1}"#, "unknown field"),
			(br#"{}"#, "no events array"),
		] {
			assert!(batch(body).unwrap_err().0.contains(want), "{}", String::from_utf8_lossy(body));
		}
		let big = format!(r#"{{"events":[{}]}}"#, vec!["{}"; 501].join(","));
		assert!(batch(big.as_bytes()).is_err());
	}
}
