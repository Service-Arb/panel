//! Roles inside the panel (spec §5.4), and how one is read off the caller's identity in
//! concierge: their grant on the scope `allocation:service_arb`, or a global admin/owner
//! role standing in for one.
//!
//! There is no read-only role: someone who only looks is an ordinary user without a grant,
//! and a scoped service does not let them in at all.

use std::str::FromStr;

use crate::Invalid;

/// Ordered by what they may do: each role may do everything the one before it may.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Role {
	Operator,
	Admin,
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

	/// A lead's name and phone. Every role works leads, so every role sees them.
	pub fn sees_pii(self) -> bool {
		match self {
			Self::Operator | Self::Admin => true,
		}
	}

	/// Moving a lead through its stages, logging calls, entering payments.
	pub fn edits_leads(self) -> bool {
		match self {
			Self::Operator | Self::Admin => true,
		}
	}

	/// Sources, their HMAC keys, and everyone's notification rules.
	pub fn manages_sources(self) -> bool {
		match self {
			Self::Operator => false,
			Self::Admin => true,
		}
	}

	/// A place's live settings (its phones route the money), withdrawing and restoring it,
	/// registering one by hand. Every role reads them.
	pub fn edits_places(self) -> bool {
		match self {
			Self::Operator => false,
			Self::Admin => true,
		}
	}

	/// A brand's price list (what its sites quote customers), and taking it off the sites.
	/// Every role reads it and previews a price.
	pub fn edits_pricing(self) -> bool {
		match self {
			Self::Operator => false,
			Self::Admin => true,
		}
	}

	/// A brand's experiments: switching one off, its weights, its holdout (what its sites
	/// show whom). Every role reads them.
	pub fn edits_experiments(self) -> bool {
		match self {
			Self::Operator => false,
			Self::Admin => true,
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
		let table = [
			(Role::Operator, [true, true, false, false], GrafanaRole::Viewer),
			(Role::Admin, [true, true, true, true], GrafanaRole::Editor),
		];
		for (role, [pii, edits, sources, places], grafana) in table {
			assert_eq!(
				[role.sees_pii(), role.edits_leads(), role.manages_sources(), role.edits_places()],
				[pii, edits, sources, places],
				"{role:?}"
			);
			assert_eq!(role.grafana(), grafana);
			assert_eq!(role.edits_pricing(), places, "pricing is edited by whoever edits places");
			assert_eq!(role.as_str().parse::<Role>().unwrap(), role);
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
