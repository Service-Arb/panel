//! `nix run .#gen`: the TS the front end would otherwise re-type from the Rust. The output is
//! committed, and the `generated` pre-commit hook re-runs this and re-stages it.

use std::{collections::BTreeMap, path::Path};

use ev_lib::ts_gen::Ts;
use panel_core::role::{Permission, Role};
use panel_server::signin::Caller;
use strum::IntoEnumIterator;

fn main() {
	let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
	let roles: Vec<Role> = Role::iter().collect();
	let may: BTreeMap<Role, BTreeMap<Permission, bool>> = Role::iter().map(|r| (r, Permission::iter().map(|p| (p, r.may(p))).collect())).collect();
	Ts::write(
		&root.join("frontend/src/entities/session/model/generated.ts"),
		&[
			Ts::types::<Caller>(),
			Ts::Value {
				name: "ROLES",
				value: serde_json::to_value(roles).expect("a unit variant serializes to its name"),
			},
			Ts::Value {
				name: "MAY",
				value: serde_json::to_value(may).expect("a unit variant is a valid map key"),
			},
		],
	);
}
