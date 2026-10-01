//! The aggregate stages of the funnel (spec §2, stages 3–4) and the experiments' daily
//! tallies, as PostHog counts them: one number per UTC day and slice, never a person.
//!
//! A count of a day is not final while the day is recent — events arrive late — so the
//! importer counts the last days again every hour. The journal is append-only, so a new
//! count is a new event of the same slice with the next `revision`, written only when the
//! count changed; the projection keeps the highest revision of each slice.

use std::collections::BTreeMap;

use jiff::civil::Date;

use crate::{Invalid, ids::is_slug};

/// How a visitor reached for the business without (yet) leaving a lead — kitstart's
/// `contact_intent_click{channel}` and the experiments' `experiment_contact{channel}`.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum IntentChannel {
	/// A tap on a `tel:` link.
	Phone,
	/// A tap on a `wa.me` link.
	Whatsapp,
	/// A link or button that opens the quote form.
	FormOpen,
	/// A link or button that opens a booking.
	Booking,
}

impl IntentChannel {
	pub const ALL: [Self; 4] = [Self::Phone, Self::Whatsapp, Self::FormOpen, Self::Booking];

	pub fn as_str(self) -> &'static str {
		match self {
			Self::Phone => "phone",
			Self::Whatsapp => "whatsapp",
			Self::FormOpen => "form_open",
			Self::Booking => "booking",
		}
	}

	pub fn parse(raw: &str) -> Result<Self, Invalid> {
		Self::ALL
			.into_iter()
			.find(|c| c.as_str() == raw)
			.ok_or_else(|| Invalid::new("properties.channel is not one of phone, whatsapp, form_open, booking"))
	}
}

/// One experiment variant's day: how many page views were exposed to it, and what came of
/// them. Exposures and what follows are counted apart, per page view, and never joined per
/// person — the landings' analytics are cookieless.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Tally {
	pub exposures: u64,
	/// Leads the landing's store accepted (`experiment_lead`, sent server side).
	pub leads: u64,
	pub phone: u64,
	pub whatsapp: u64,
	pub form_open: u64,
	pub booking: u64,
}

impl Tally {
	/// The intents of a channel.
	pub fn intents(&self, channel: IntentChannel) -> u64 {
		match channel {
			IntentChannel::Phone => self.phone,
			IntentChannel::Whatsapp => self.whatsapp,
			IntentChannel::FormOpen => self.form_open,
			IntentChannel::Booking => self.booking,
		}
	}

	pub fn intents_mut(&mut self, channel: IntentChannel) -> &mut u64 {
		match channel {
			IntentChannel::Phone => &mut self.phone,
			IntentChannel::Whatsapp => &mut self.whatsapp,
			IntentChannel::FormOpen => &mut self.form_open,
			IntentChannel::Booking => &mut self.booking,
		}
	}

	/// The experiments' contact metric, as both landings' reports define it: everything that
	/// reaches the business — leads, calls and WhatsApp taps (docs/EXPERIMENTS.md there).
	pub fn contacts(&self) -> u64 {
		self.leads.saturating_add(self.phone).saturating_add(self.whatsapp)
	}

	pub fn add(&mut self, other: &Self) {
		self.exposures = self.exposures.saturating_add(other.exposures);
		self.leads = self.leads.saturating_add(other.leads);
		for c in IntentChannel::ALL {
			*self.intents_mut(c) = self.intents(c).saturating_add(other.intents(c));
		}
	}
}

/// What a day's count is of.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MetricValue {
	/// `site.metrics`: page views of a location from one traffic source (stage 3).
	Visits { source: String, visits: u64 },
	/// `contact.metrics`: intents through one channel at a location (stage 4).
	Intents { channel: IntentChannel, intents: u64 },
	/// `experiment.metrics`: one variant of one experiment of a brand.
	Experiment { experiment: String, variant: String, tally: Tally },
}

/// A count of one UTC day and slice, and which count of it this is.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DailyMetric {
	pub day: Date,
	/// 1, 2, …: the projection keeps the highest.
	pub revision: u32,
	pub value: MetricValue,
}

impl DailyMetric {
	pub fn parse_day(raw: &str) -> Result<Date, Invalid> {
		// `YYYY-MM-DD` exactly: a day is a civil date in UTC, never a time.
		if raw.len() != 10 {
			return Err(Invalid::new("properties.day is not a day like \"2026-09-30\""));
		}
		raw.parse::<Date>().map_err(|_| Invalid::new("properties.day is not a day like \"2026-09-30\""))
	}

	pub fn revision(raw: u32) -> Result<u32, Invalid> {
		// The projection keeps it in a Postgres integer.
		if raw == 0 || raw > i32::MAX as u32 {
			return Err(Invalid::new("properties.revision must be 1 to 2147483647"));
		}
		Ok(raw)
	}

	/// A count from the wire: an int64 that must not be negative.
	pub fn count(field: &str, raw: i64) -> Result<u64, Invalid> {
		u64::try_from(raw).map_err(|_| Invalid::new(format!("properties.{field} is negative")))
	}

	pub fn source(raw: &str) -> Result<String, Invalid> {
		if is_source(raw) {
			Ok(raw.to_owned())
		} else {
			Err(Invalid::new("properties.source is not a lowercase source like \"google.com\" or \"direct\""))
		}
	}

	/// An experiment's or a variant's name: a lowercase slug.
	pub fn name(field: &str, raw: &str) -> Result<String, Invalid> {
		if is_slug(raw) {
			Ok(raw.to_owned())
		} else {
			Err(Invalid::new(format!("properties.{field} is not a lowercase slug")))
		}
	}

	/// What the slice is, apart from the day, the brand and the location: what one revision
	/// replaces another of.
	pub fn dimension(&self) -> &str {
		match &self.value {
			MetricValue::Visits { source, .. } => source,
			MetricValue::Intents { channel, .. } => channel.as_str(),
			MetricValue::Experiment { .. } => "",
		}
	}

	/// Whether the subject may carry a location: the experiments are a brand's, since the
	/// server-side `experiment_lead` names no location on every landing.
	pub fn located(&self) -> bool {
		!matches!(self.value, MetricValue::Experiment { .. })
	}
}

/// A traffic source as the projection keeps it: lowercase, 1–64 of `[a-z0-9._:-]`, starting
/// alphanumeric. What kitstart sends is `utm_source`, the referrer's host, `direct`,
/// `internal` or `unknown`.
pub fn is_source(s: &str) -> bool {
	(1..=64).contains(&s.len())
		&& s.starts_with(|c: char| c.is_ascii_lowercase() || c.is_ascii_digit())
		&& s.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'.' | b'_' | b'-' | b':'))
}

/// What does not fit is counted here: sources the projection cannot keep, and the tail past
/// [`MAX_SOURCES`].
pub const OTHER_SOURCE: &str = "other";

/// The most sources a location keeps per day; the rest are summed into [`OTHER_SOURCE`].
/// `utm_source` is whatever a link says, so the number of distinct ones is not ours to bound.
pub const MAX_SOURCES: usize = 20;

/// A source as a visitor's page reported it, made one the projection keeps: lowercased and
/// trimmed; none at all is `unknown`; anything else unfit is [`OTHER_SOURCE`].
pub fn normalize_source(raw: Option<&str>) -> String {
	let Some(s) = raw.map(|s| s.trim().to_ascii_lowercase()).filter(|s| !s.is_empty()) else {
		return "unknown".to_owned();
	};
	if is_source(&s) { s } else { OTHER_SOURCE.to_owned() }
}

/// One location's day of sources, the largest [`MAX_SOURCES`] kept and the rest summed into
/// [`OTHER_SOURCE`]. Ties go to the name, so the same counts always keep the same sources.
pub fn top_sources(counts: BTreeMap<String, u64>) -> BTreeMap<String, u64> {
	let mut ranked: Vec<(String, u64)> = counts.into_iter().collect();
	ranked.sort_by(|(a, x), (b, y)| y.cmp(x).then_with(|| a.cmp(b)));
	let mut kept = BTreeMap::new();
	let mut other = 0u64;
	for (i, (source, n)) in ranked.into_iter().enumerate() {
		if i < MAX_SOURCES && source != OTHER_SOURCE {
			kept.insert(source, n);
		} else {
			other = other.saturating_add(n);
		}
	}
	if other > 0 {
		*kept.entry(OTHER_SOURCE.to_owned()).or_insert(0) += other;
	}
	kept
}

/// What a fresh count of some days means for the journal: every slice whose count is new
/// or changed, at the revision after the one the projection has; and every slice the
/// projection has with a count that the fresh one no longer finds, back to `zero`. A slice
/// counted the same again is nothing to write.
pub fn changes<K: Ord + Clone, V: Eq + Clone>(current: &BTreeMap<K, (V, u32)>, fresh: &BTreeMap<K, V>, zero: &V) -> Vec<(K, V, u32)> {
	let mut out = Vec::new();
	for (k, v) in fresh {
		match current.get(k) {
			Some((had, _)) if had == v => {}
			Some((_, rev)) => out.push((k.clone(), v.clone(), rev.saturating_add(1))),
			None => out.push((k.clone(), v.clone(), 1)),
		}
	}
	for (k, (had, rev)) in current {
		if !fresh.contains_key(k) && had != zero {
			out.push((k.clone(), zero.clone(), rev.saturating_add(1)));
		}
	}
	out
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn days_and_names() {
		assert_eq!(DailyMetric::parse_day("2026-09-30").unwrap().to_string(), "2026-09-30");
		for bad in ["2026-9-30", "2026-09-31", "2026-09-30T00:00:00Z", ""] {
			assert!(DailyMetric::parse_day(bad).is_err(), "{bad}");
		}
		assert!(DailyMetric::revision(0).is_err());
		assert!(DailyMetric::count("visits", -1).is_err());
		assert!(DailyMetric::name("variant", "B").is_err());
		assert_eq!(IntentChannel::parse("form_open").unwrap(), IntentChannel::FormOpen);
		assert!(IntentChannel::parse("email").is_err());
	}

	#[test]
	fn sources_are_bounded() {
		assert_eq!(normalize_source(Some(" Google.com ")), "google.com");
		assert_eq!(normalize_source(None), "unknown");
		assert_eq!(normalize_source(Some("")), "unknown");
		assert_eq!(normalize_source(Some("ivan petrov")), OTHER_SOURCE, "spaces: not a source the projection keeps");
		assert_eq!(normalize_source(Some(&"a".repeat(65))), OTHER_SOURCE);

		let mut counts: BTreeMap<String, u64> = (0..25).map(|i| (format!("s{i:02}"), 100 - i)).collect();
		counts.insert(OTHER_SOURCE.into(), 7);
		let kept = top_sources(counts);
		assert_eq!(kept.len(), MAX_SOURCES + 1);
		assert_eq!(kept["s00"], 100);
		assert!(!kept.contains_key("s20"));
		// s20..s24 are 80..76, plus the 7 already "other".
		assert_eq!(kept[OTHER_SOURCE], 80 + 79 + 78 + 77 + 76 + 7);
	}

	#[test]
	fn only_what_changed_is_written() {
		let current: BTreeMap<&str, (u64, u32)> = [("same", (5, 1)), ("grew", (5, 2)), ("gone", (3, 1)), ("gone_zero", (0, 4))].into();
		let fresh: BTreeMap<&str, u64> = [("same", 5), ("grew", 6), ("new", 1)].into();
		let mut got = changes(&current, &fresh, &0);
		got.sort();
		assert_eq!(got, [("gone", 0, 2), ("grew", 6, 3), ("new", 1, 1)]);
	}

	#[test]
	fn the_contact_metric() {
		let t = Tally {
			exposures: 100,
			leads: 2,
			phone: 3,
			whatsapp: 1,
			form_open: 9,
			booking: 4,
		};
		assert_eq!(t.contacts(), 6, "form opens and bookings are intents, not contacts");
		let mut sum = t;
		sum.add(&t);
		assert_eq!((sum.exposures, sum.booking), (200, 8));
	}
}
