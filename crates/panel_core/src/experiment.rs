//! What the experiments screen may say about a variant against its control (spec §10.1):
//! the difference of two rates and its 95 % interval, and "too little data" while that
//! interval holds zero or an arm is too small. Never a winner.
//!
//! **The interval is Newcombe's hybrid score interval** for a difference of two independent
//! proportions (Newcombe 1998, method 10), built from the two Wilson score intervals. The
//! Wald interval `d ± z·SE` is what a textbook gives first and is wrong exactly where the
//! landings live: rates of a few percent and arms of a few hundred page views — it collapses
//! to a point at zero successes, overshoots [−1, 1], and covers far less than 95 % for small
//! rates. Newcombe's stays within bounds, is defined at 0 and at n, and keeps its coverage
//! close to nominal at small counts, at the cost of a square root.
//!
//! What it assumes, and the landings only approximately give: each exposure (a page view)
//! converts or not, independently. A visitor who views two pages is two exposures, and a
//! page view that taps twice is capped at one success — the counts are per page view, as
//! both landings' own reports treat them.

/// The normal quantile of a two-sided 95 % interval.
pub const Z95: f64 = 1.959_963_984_540_054;

/// An arm needs this many exposures before its difference to the control is shown at all —
/// the landings' stop rule (docs/EXPERIMENTS.md there: "≥ ~100 exposures in each arm").
pub const MIN_EXPOSURES: u64 = 100;

/// A closed interval of a rate or of a difference of rates, as fractions.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Interval {
	pub low: f64,
	pub high: f64,
}

impl Interval {
	pub fn contains_zero(&self) -> bool {
		self.low <= 0.0 && self.high >= 0.0
	}
}

/// The Wilson score interval of `x` successes in `n` trials; `None` without a trial. `x` past
/// `n` counts as `n`.
pub fn wilson(x: u64, n: u64, z: f64) -> Option<Interval> {
	if n == 0 {
		return None;
	}
	let (x, n) = (x.min(n) as f64, n as f64);
	let p = x / n;
	let z2 = z * z;
	let denom = 1.0 + z2 / n;
	let centre = (p + z2 / (2.0 * n)) / denom;
	let half = z / denom * (p * (1.0 - p) / n + z2 / (4.0 * n * n)).sqrt();
	Some(Interval {
		low: (centre - half).max(0.0),
		high: (centre + half).min(1.0),
	})
}

/// Successes out of trials; successes past the trials are capped (see the module docs).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Arm {
	pub successes: u64,
	pub trials: u64,
}

impl Arm {
	pub fn new(successes: u64, trials: u64) -> Self {
		Self {
			successes: successes.min(trials),
			trials,
		}
	}

	fn rate(&self) -> f64 {
		self.successes as f64 / self.trials as f64
	}
}

/// `treatment − control` and Newcombe's interval of it; `None` when an arm has no trial.
pub fn newcombe(control: Arm, treatment: Arm, z: f64) -> Option<(f64, Interval)> {
	let (c, t) = (wilson(control.successes, control.trials, z)?, wilson(treatment.successes, treatment.trials, z)?);
	let (pc, pt) = (control.rate(), treatment.rate());
	let d = pt - pc;
	let low = d - ((pt - t.low).powi(2) + (c.high - pc).powi(2)).sqrt();
	let high = d + ((t.high - pt).powi(2) + (pc - c.low).powi(2)).sqrt();
	Some((d, Interval { low, high }))
}

/// Why a difference is not one to act on.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Insufficient {
	/// An arm has fewer than [`MIN_EXPOSURES`]: no difference is given.
	SmallSample,
	/// The interval holds zero: the variants are not told apart.
	IntervalIncludesZero,
}

impl Insufficient {
	pub fn as_str(self) -> &'static str {
		match self {
			Self::SmallSample => "small_sample",
			Self::IntervalIncludesZero => "interval_includes_zero",
		}
	}
}

/// A difference in percentage points, rounded no finer than the data: whole points while the
/// interval is 2 points wide or more, a tenth of a point when it is narrower.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Points {
	pub estimate: f64,
	pub low: f64,
	pub high: f64,
	pub decimals: u8,
}

impl Points {
	fn of(d: f64, i: Interval) -> Self {
		let (estimate, low, high) = (d * 100.0, i.low * 100.0, i.high * 100.0);
		let decimals = if high - low >= 2.0 { 0 } else { 1 };
		let scale = if decimals == 0 { 1.0 } else { 10.0 };
		let round = |v: f64| {
			let r = (v * scale).round() / scale;
			// `-0` reads as a sign the data does not have.
			if r == 0.0 { 0.0 } else { r }
		};
		Self {
			estimate: round(estimate),
			low: round(low),
			high: round(high),
			decimals,
		}
	}
}

/// A variant against the control on one metric.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Comparison {
	/// `None` while an arm is under [`MIN_EXPOSURES`].
	pub difference: Option<Points>,
	/// `None` when the interval excludes zero; even then, the screen says only which way
	/// and how far, not that a variant won.
	pub insufficient: Option<Insufficient>,
}

/// Compares `treatment` with `control` at 95 %. Zero inside the interval is judged on the
/// unrounded bounds.
pub fn compare(control: Arm, treatment: Arm) -> Comparison {
	if control.trials < MIN_EXPOSURES || treatment.trials < MIN_EXPOSURES {
		return Comparison {
			difference: None,
			insufficient: Some(Insufficient::SmallSample),
		};
	}
	match newcombe(control, treatment, Z95) {
		Some((d, i)) => Comparison {
			difference: Some(Points::of(d, i)),
			insufficient: i.contains_zero().then_some(Insufficient::IntervalIncludesZero),
		},
		None => Comparison {
			difference: None,
			insufficient: Some(Insufficient::SmallSample),
		},
	}
}

/// Which variant is the control: `@evinvest/experiments` puts it first in the landing's
/// config, which PostHog never sees; by convention it is `control` or `a`, else the first by
/// name.
pub fn control_of<'a>(variants: impl IntoIterator<Item = &'a str>) -> Option<&'a str> {
	let mut all: Vec<&str> = variants.into_iter().collect();
	all.sort_unstable();
	["control", "a"].into_iter().find_map(|c| all.iter().copied().find(|v| *v == c)).or_else(|| all.first().copied())
}

#[cfg(test)]
mod tests {
	use super::*;

	fn close(got: f64, want: f64) {
		assert!((got - want).abs() < 5e-5, "{got} is not {want}");
	}

	/// Newcombe, "Two-sided confidence intervals for the single proportion" (1998), Table I,
	/// method 3.
	#[test]
	fn wilson_on_the_published_examples() {
		for (x, n, low, high) in [(81, 263, 0.2553, 0.3662), (15, 148, 0.0624, 0.1605), (0, 20, 0.0, 0.1611), (1, 29, 0.0061, 0.1718)] {
			let i = wilson(x, n, Z95).unwrap();
			close(i.low, low);
			close(i.high, high);
		}
		assert_eq!(wilson(1, 0, Z95), None);
		assert_eq!(wilson(5, 3, Z95), wilson(3, 3, Z95), "capped at n");
	}

	/// Newcombe, "Interval estimation for the difference between independent proportions"
	/// (1998), Table II, method 10: there `p1 − p2`, here `treatment − control`, so the
	/// first arm is the treatment.
	#[test]
	fn newcombe_on_the_published_examples() {
		for ((x1, n1), (x2, n2), low, high) in [
			((56, 70), (48, 80), 0.0524, 0.3339),
			((9, 10), (3, 10), 0.1705, 0.8090),
			((6, 7), (2, 7), 0.0582, 0.8062),
			((5, 56), (0, 29), -0.0381, 0.1926),
			((0, 10), (0, 20), -0.1611, 0.2775),
			((10, 10), (0, 20), 0.6791, 1.0),
		] {
			let (_, i) = newcombe(Arm::new(x2, n2), Arm::new(x1, n1), Z95).unwrap();
			close(i.low, low);
			close(i.high, high);
		}
	}

	#[test]
	fn no_winner_on_little_data() {
		let small = compare(Arm::new(1, 40), Arm::new(9, 40));
		assert_eq!(
			small,
			Comparison {
				difference: None,
				insufficient: Some(Insufficient::SmallSample)
			},
			"40 page views an arm is too few, however different"
		);

		let even = compare(Arm::new(10, 500), Arm::new(14, 500));
		assert_eq!(even.insufficient, Some(Insufficient::IntervalIncludesZero));
		let d = even.difference.unwrap();
		assert!(d.low < 0.0 && d.high > 0.0, "{d:?}");
		assert_eq!(d.decimals, 0, "an interval this wide is whole points");

		let apart = compare(Arm::new(20, 1000), Arm::new(60, 1000));
		assert_eq!(apart.insufficient, None);
		let d = apart.difference.unwrap();
		assert_eq!((d.estimate, d.decimals), (4.0, 0));
		assert!(d.low > 0.0, "{d:?}");

		let narrow = compare(Arm::new(1000, 100_000), Arm::new(1300, 100_000));
		let d = narrow.difference.unwrap();
		assert_eq!((d.estimate, d.decimals), (0.3, 1), "{d:?}");
		assert_eq!(narrow.insufficient, None);
	}

	#[test]
	fn the_control() {
		assert_eq!(control_of(["b", "a"]), Some("a"));
		assert_eq!(control_of(["test", "control", "a"]), Some("control"));
		assert_eq!(control_of(["single", "double"]), Some("double"));
		assert_eq!(control_of([]), None);
	}
}
