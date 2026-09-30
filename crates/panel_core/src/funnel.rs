//! What the funnel screens may say, and no more precisely than the data allows (spec §10.1).

use jiff::{SignedDuration, Timestamp};

use crate::lead::Stage;

/// Below this many in the denominator, a share is `n of m`, never a percent.
pub const MIN_SAMPLE: u64 = 30;

/// How long a new lead may wait for its first contact before it is overdue.
pub const CONTACT_SLA: SignedDuration = SignedDuration::from_mins(30);

/// `n` out of `of`. The percent is there only when `of` is at least [`MIN_SAMPLE`], and then
/// whole: with fewer, "12 of 17" is all the data says, and 70.6 % would claim more.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Share {
	pub n: u64,
	pub of: u64,
	pub percent: Option<u64>,
}

impl Share {
	pub fn new(n: u64, of: u64) -> Self {
		let percent = (of >= MIN_SAMPLE).then(|| (n.saturating_mul(100) + of / 2) / of);
		Self { n, of, percent }
	}

	pub fn small_sample(&self) -> bool {
		self.percent.is_none()
	}
}

/// Leads that came in over some days, by how far each got (the columns of
/// `reporting.funnel_daily`, summed).
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Totals {
	pub leads: u64,
	pub contacted: u64,
	pub quoted: u64,
	pub won: u64,
	pub completed: u64,
	pub paid: u64,
	/// Lost now, whatever stage they had reached.
	pub lost: u64,
	/// Typed in by hand (§10a).
	pub manual: u64,
}

/// One stage of the personal funnel (5–10): how many reached it, as a share of the stage
/// before and of all the leads.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StageStep {
	pub stage: Stage,
	pub reached: u64,
	/// `None` for the first stage, which has no stage before it.
	pub of_previous: Option<Share>,
	pub of_leads: Share,
}

impl Totals {
	pub fn steps(&self) -> Vec<StageStep> {
		let reached = [
			(Stage::Created, self.leads),
			(Stage::Contacted, self.contacted),
			(Stage::Quoted, self.quoted),
			(Stage::Won, self.won),
			(Stage::Completed, self.completed),
			(Stage::Paid, self.paid),
		];
		let mut previous = None;
		reached
			.into_iter()
			.map(|(stage, n)| {
				let step = StageStep {
					stage,
					reached: n,
					of_previous: previous.map(|p| Share::new(n, p)),
					of_leads: Share::new(n, self.leads),
				};
				previous = Some(n);
				step
			})
			.collect()
	}
}

/// A lead still waiting for its first contact: since when, and whether past [`CONTACT_SLA`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Waiting {
	pub since: Timestamp,
	pub overdue: bool,
}

/// Whether a lead waits for contact: it came in and has neither been contacted nor gone
/// further (nor been lost).
pub fn waiting(stage: Stage, created: Option<Timestamp>, contacted: Option<Timestamp>, now: Timestamp) -> Option<Waiting> {
	match (stage, created, contacted) {
		(Stage::Created, Some(since), None) => Some(Waiting {
			since,
			overdue: now.duration_since(since) > CONTACT_SLA,
		}),
		_ => None,
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn small_samples_have_no_percent() {
		let s = Share::new(12, 17);
		assert_eq!((s.n, s.of, s.percent), (12, 17, None));
		assert!(s.small_sample());
		assert_eq!(Share::new(0, 0).percent, None);
		assert_eq!(Share::new(10, 29).percent, None, "29 is still too few");
		assert_eq!(Share::new(10, 30).percent, Some(33), "whole percent, rounded");
		assert_eq!(Share::new(1, 40).percent, Some(3), "2.5 rounds up");
		assert!(!Share::new(30, 30).small_sample());
	}

	#[test]
	fn steps_are_shares_of_the_stage_before() {
		let t = Totals {
			leads: 100,
			contacted: 80,
			quoted: 20,
			won: 10,
			completed: 9,
			paid: 9,
			lost: 30,
			manual: 5,
		};
		let steps = t.steps();
		assert_eq!(
			steps.iter().map(|s| s.stage).collect::<Vec<_>>(),
			[Stage::Created, Stage::Contacted, Stage::Quoted, Stage::Won, Stage::Completed, Stage::Paid]
		);
		assert_eq!(steps[0].of_previous, None);
		assert_eq!(steps[1].of_previous.unwrap().percent, Some(80));
		assert_eq!(steps[2].of_previous.unwrap().percent, Some(25));
		assert_eq!(steps[3].of_previous.unwrap(), Share { n: 10, of: 20, percent: None }, "20 quoted is too few to divide");
		assert_eq!(steps[3].of_leads.percent, Some(10));
	}

	#[test]
	fn waiting_for_contact() {
		let t0: Timestamp = "2026-09-30T10:00:00Z".parse().unwrap();
		let at = |mins| t0 + SignedDuration::from_mins(mins);
		assert_eq!(waiting(Stage::Created, Some(t0), None, at(30)), Some(Waiting { since: t0, overdue: false }));
		assert!(waiting(Stage::Created, Some(t0), None, at(31)).unwrap().overdue);
		assert_eq!(waiting(Stage::Contacted, Some(t0), Some(at(1)), at(60)), None);
		assert_eq!(waiting(Stage::Lost, Some(t0), None, at(60)), None, "a lost lead waits for nothing");
		assert_eq!(waiting(Stage::Created, None, None, at(60)), None, "seen before its creation");
	}
}
