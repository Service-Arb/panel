//! Roles inside the panel (spec §5.4), and how one is read off the caller's identity in
//! concierge: their grant on the scope `allocation:service_arb`, or a global admin/owner
//! role standing in for one.
//!
//! There is no read-only role: someone who only looks is an ordinary user without a grant,
//! and a scoped service does not let them in at all.

use std::str::FromStr;

use serde::Serialize;
use strum::EnumIter;
use ts_rs::TS;

use crate::Invalid;

/// Ordered by what they may do: each role may do everything the one before it may.
#[derive(Clone, Copy, Debug, EnumIter, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum Role {
	Operator,
	Admin,
}

/// What a role may do (spec §5.4), [`Role::may`]. Every role reads everything it is let into.
#[derive(Clone, Copy, Debug, EnumIter, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Permission {
	/// A lead's name and phone.
	SeesPii,
	/// Moving a lead through its stages, logging calls, entering payments.
	EditsLeads,
	/// Sources, their HMAC keys, and everyone's notification rules.
	ManagesSources,
	/// A place's live settings (its phones route the money), withdrawing and restoring it,
	/// registering one by hand.
	EditsPlaces,
	/// A brand's price list (what its sites quote customers), and taking it off the sites.
	EditsPricing,
	/// A brand's experiments: switching one off, its weights, its holdout (what its sites
	/// show whom).
	EditsExperiments,
}

/// The org role the panel's Grafana gets through `X-WEBAUTH-ROLE` (spec §6).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GrafanaRole {
	Viewer,
	Editor,
}

/// The concierge scope a panel role is granted on.
pub const SCOPE: &str = "allocation:service_arb";

impl Role {
	/// The caller's role in the panel, from concierge's `GetMe`: `global` is their platform
	/// role (investor/operator/admin/owner), `grants` their scoped grants as
	/// `(scope, role)`. A grant on [`SCOPE`] gives its role; a global `admin` or `owner`
	/// counts as a panel admin with or without one (concierge admits them to the client on
	/// the same rule). The higher of the two wins. `None`: not let in.
	///
	/// A global `operator` is not a panel operator: that role is about the fund, and this
	/// allocation's team is whoever its grants name.
	pub fn admitted<'a>(global: &str, grants: impl IntoIterator<Item = (&'a str, &'a str)>) -> Option<Self> {
		let scoped = grants.into_iter().filter(|(scope, _)| *scope == SCOPE).filter_map(|(_, role)| role.parse::<Self>().ok()).max();
		let global = matches!(global, "admin" | "owner").then_some(Self::Admin);
		scoped.max(global)
	}

	pub fn as_str(self) -> &'static str {
		match self {
			Self::Operator => "operator",
			Self::Admin => "admin",
		}
	}

	pub fn may(self, what: Permission) -> bool {
		self >= match what {
			Permission::SeesPii | Permission::EditsLeads => Self::Operator,
			Permission::ManagesSources | Permission::EditsPlaces | Permission::EditsPricing | Permission::EditsExperiments => Self::Admin,
		}
	}

	pub fn grafana(self) -> GrafanaRole {
		match self {
			Self::Operator => GrafanaRole::Viewer,
			Self::Admin => GrafanaRole::Editor,
		}
	}
}

impl FromStr for Role {
	type Err = Invalid;

	fn from_str(s: &str) -> Result<Self, Invalid> {
		match s {
			"operator" => Ok(Self::Operator),
			"admin" => Ok(Self::Admin),
			other => Err(Invalid::new(format!("{other:?} is not one of operator, admin"))),
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	/// The table of spec §5.4, row by row.
	#[test]
	fn the_spec_table() {
		use Permission::*;
		let table = [
			(Role::Operator, [true, true, false, false, false, false], GrafanaRole::Viewer),
			(Role::Admin, [true, true, true, true, true, true], GrafanaRole::Editor),
		];
		for (role, may, grafana) in table {
			assert_eq!(
				[SeesPii, EditsLeads, ManagesSources, EditsPlaces, EditsPricing, EditsExperiments].map(|p| role.may(p)),
				may,
				"{role:?}"
			);
			assert_eq!(role.grafana(), grafana);
			assert_eq!(role.as_str().parse::<Role>().unwrap(), role);
			assert_eq!(serde_json::to_value(role).unwrap(), role.as_str(), "the wire's word is the parsed one");
		}
		assert!("viewer".parse::<Role>().is_err(), "the owner dropped it (2026-09-30)");
		assert!("owner".parse::<Role>().is_err());
	}

	#[test]
	fn admission() {
		let ours = |role| (SCOPE, role);
		assert_eq!(Role::admitted("investor", [ours("operator")]), Some(Role::Operator));
		assert_eq!(Role::admitted("investor", [ours("admin"), ours("operator")]), Some(Role::Admin));
		assert_eq!(Role::admitted("admin", []), Some(Role::Admin), "a global admin without a grant");
		assert_eq!(Role::admitted("owner", [ours("operator")]), Some(Role::Admin), "the higher of the two");
		assert_eq!(Role::admitted("operator", []), None, "a fund operator is not this allocation's");
		assert_eq!(Role::admitted("investor", [("allocation:real_estate", "admin")]), None, "another allocation's grant");
		assert_eq!(Role::admitted("investor", [ours("viewer")]), None, "no such role any more");
		assert_eq!(Role::admitted("investor", []), None);
	}
}
