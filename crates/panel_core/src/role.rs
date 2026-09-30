//! Roles inside the panel (spec §5.4). A role comes from the caller's scope grant
//! `allocation:service_arb` in concierge; signing in is not built yet, so for now this is
//! only the table of what each role may do.

use std::str::FromStr;

use crate::Invalid;

/// Ordered by what they may do: each role may do everything the one before it may.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Role {
	Viewer,
	Operator,
	Admin,
}

/// The org role the panel's Grafana gets through `X-WEBAUTH-ROLE` (spec §6).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GrafanaRole {
	Viewer,
	Editor,
}

impl Role {
	pub fn as_str(self) -> &'static str {
		match self {
			Self::Viewer => "viewer",
			Self::Operator => "operator",
			Self::Admin => "admin",
		}
	}

	/// The funnel and the reports.
	pub fn sees_funnel(self) -> bool {
		true
	}

	/// A lead's name and phone.
	pub fn sees_pii(self) -> bool {
		self >= Self::Operator
	}

	/// Moving a lead through its stages, logging calls, entering payments.
	pub fn edits_leads(self) -> bool {
		self >= Self::Operator
	}

	/// Sources, their HMAC keys, and everyone's notification rules.
	pub fn manages_sources(self) -> bool {
		self >= Self::Admin
	}

	pub fn grafana(self) -> GrafanaRole {
		match self {
			Self::Viewer | Self::Operator => GrafanaRole::Viewer,
			Self::Admin => GrafanaRole::Editor,
		}
	}
}

impl FromStr for Role {
	type Err = Invalid;

	fn from_str(s: &str) -> Result<Self, Invalid> {
		match s {
			"viewer" => Ok(Self::Viewer),
			"operator" => Ok(Self::Operator),
			"admin" => Ok(Self::Admin),
			other => Err(Invalid::new(format!("{other:?} is not one of viewer, operator, admin"))),
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
			(Role::Viewer, [true, false, false, false], GrafanaRole::Viewer),
			(Role::Operator, [true, true, true, false], GrafanaRole::Viewer),
			(Role::Admin, [true, true, true, true], GrafanaRole::Editor),
		];
		for (role, [funnel, pii, edits, sources], grafana) in table {
			assert_eq!(
				[role.sees_funnel(), role.sees_pii(), role.edits_leads(), role.manages_sources()],
				[funnel, pii, edits, sources],
				"{role:?}"
			);
			assert_eq!(role.grafana(), grafana);
			assert_eq!(role.as_str().parse::<Role>().unwrap(), role);
		}
		assert!("owner".parse::<Role>().is_err());
	}
}
