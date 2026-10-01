//! What the screens read of the PostHog counts: the aggregate stages (3–4) beside the funnel,
//! by day, and the experiments' variants summed over a window. Aggregates only: nothing here
//! is joined to a lead (spec §10.1).

use std::collections::BTreeMap;

use eyre::WrapErr;
use jiff::civil::Date;
use panel_core::{experiment::control_of, ids::BrandId, metrics::IntentChannel};

use crate::{
	Panel,
	operator::FunnelBy,
	store::metrics::{self as db, CONTACT_INTENT, VISITS, VariantTotals},
};

/// One UTC day of a slice's aggregate stages.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SiteDay {
	pub visits: u64,
	pub intents: BTreeMap<IntentChannel, u64>,
}

/// A slice's aggregate stages over a window: per day (the days with a count only), and its
/// visits by source over the whole window.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SiteSlice {
	/// `None` for [`FunnelBy::All`].
	pub brand: Option<String>,
	/// `None` for [`FunnelBy::All`], and for the pages that name no location.
	pub location: Option<String>,
	pub days: BTreeMap<Date, SiteDay>,
	pub sources: BTreeMap<String, u64>,
}

/// An experiment of a brand: its variants summed over the window, the control first.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExperimentView {
	pub brand: String,
	pub experiment: String,
	pub first_day: Date,
	pub last_day: Date,
	pub control: String,
	pub variants: Vec<VariantTotals>,
}

impl Panel {
	/// The aggregate stages from `from` to `to` (UTC days, both included), cut `by` as the
	/// funnel is. With [`FunnelBy::All`], a single slice, empty included.
	pub async fn site_slices(&self, from: Date, to: Date, brand: Option<&BrandId>, by: FunnelBy) -> eyre::Result<Vec<SiteSlice>> {
		let mut conn = self.store.pool().acquire().await.wrap_err("a connection")?;
		let rows = db::location_days(&mut conn, from, to, brand).await?;
		drop(conn);
		let mut slices: BTreeMap<(Option<String>, Option<String>), SiteSlice> = BTreeMap::new();
		if by == FunnelBy::All {
			slices.insert((None, None), SiteSlice::default());
		}
		for r in rows {
			let key = match by {
				FunnelBy::All => (None, None),
				FunnelBy::Location => (Some(r.brand_id), r.location_id),
			};
			let slice = slices.entry(key.clone()).or_insert_with(|| SiteSlice {
				brand: key.0,
				location: key.1,
				..SiteSlice::default()
			});
			let day = slice.days.entry(r.day).or_default();
			match r.metric {
				VISITS => {
					day.visits += r.value;
					*slice.sources.entry(r.dimension).or_insert(0) += r.value;
				}
				CONTACT_INTENT => *day.intents.entry(IntentChannel::parse(&r.dimension)?).or_insert(0) += r.value,
				other => eyre::bail!("a count of {other}"),
			}
		}
		let mut slices: Vec<SiteSlice> = slices.into_values().collect();
		// As the funnel's: by brand and location, the pages naming none last.
		slices.sort_by(|a, b| (&a.brand, a.location.is_none(), &a.location).cmp(&(&b.brand, b.location.is_none(), &b.location)));
		Ok(slices)
	}

	/// Every experiment counted from `from` to `to` (UTC days, both included), by brand and
	/// name.
	pub async fn experiments(&self, from: Date, to: Date, brand: Option<&BrandId>) -> eyre::Result<Vec<ExperimentView>> {
		let mut conn = self.store.pool().acquire().await.wrap_err("a connection")?;
		let rows = db::variant_totals(&mut conn, from, to, brand).await?;
		drop(conn);
		let mut by_name: BTreeMap<(String, String), Vec<VariantTotals>> = BTreeMap::new();
		for r in rows {
			by_name.entry((r.brand_id.clone(), r.experiment.clone())).or_default().push(r);
		}
		Ok(by_name
			.into_iter()
			.filter_map(|((brand, experiment), mut variants)| {
				let control = control_of(variants.iter().map(|v| v.variant.as_str()))?.to_owned();
				variants.sort_by_key(|v| (v.variant != control, v.variant.clone()));
				Some(ExperimentView {
					first_day: variants.iter().map(|v| v.first_day).min()?,
					last_day: variants.iter().map(|v| v.last_day).max()?,
					brand,
					experiment,
					control,
					variants,
				})
			})
			.collect())
	}
}
