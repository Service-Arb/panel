//! The environment: secrets and the deployment profile. Where to listen is a flag.

use panel::seal::DataKey;

ev_lib::settings! {
	/// Each secret is needed only by what uses it; a missing one fails that, with an error
	/// naming it, rather than the boot — except in production, where the only thing this
	/// binary runs is `serve`, which needs both.
	pub struct Settings {
		/// The panel's Postgres, as the runtime role (`deploy/panel_app.sql`): it reads and
		/// writes, and refuses to start on a database that lacks a migration.
		#[secret]
		#[required_in("production")]
		database_url: Option<String>,
		/// The same database as the role that owns the schema: only `panel migrate` uses it,
		/// so only the migration job's environment carries it.
		#[secret]
		migrate_database_url: Option<String>,
		/// 64 hex characters: the key PII and the sources' HMAC secrets are sealed with
		/// (`panel gen-data-key` makes one). Losing it loses both; changing it strands them.
		#[secret]
		#[required_in("production")]
		panel_data_key: Option<String>,
		/// Unset: errors are logged, not reported.
		#[secret]
		sentry_dsn: Option<String>,
		app_env: String = "development",
	}
}

impl Settings {
	pub fn database_url(&self) -> eyre::Result<&str> {
		self.database_url.as_deref().ok_or_else(|| eyre::eyre!("DATABASE_URL must be set"))
	}

	pub fn migrate_database_url(&self) -> eyre::Result<&str> {
		self.migrate_database_url.as_deref().ok_or_else(|| eyre::eyre!("MIGRATE_DATABASE_URL must be set for migrate"))
	}

	pub fn data_key(&self) -> eyre::Result<DataKey> {
		let hex = self.panel_data_key.as_deref().ok_or_else(|| eyre::eyre!("PANEL_DATA_KEY must be set"))?;
		Ok(DataKey::from_hex(hex)?)
	}
}

/// `--print-required-vars[=PROFILE]` (default `production`): the variables a deploy into
/// that profile must provide, one per line, for the gitops preflight. Read before clap,
/// which would otherwise demand a subcommand.
pub fn print_required_vars_for() -> Option<String> {
	const FLAG: &str = "--print-required-vars";
	let mut args = std::env::args().skip(1);
	let arg = args.next()?;
	match arg.split_once('=') {
		Some((FLAG, profile)) => Some(profile.to_owned()),
		Some(_) => None,
		None if arg == FLAG => Some(args.next().unwrap_or_else(|| "production".to_owned())),
		None => None,
	}
}

#[cfg(test)]
mod tests {
	use std::collections::HashMap;

	use super::*;

	/// The env surface is the deploy contract (the image env and the cluster Secret use these
	/// names); a rename here must be deliberate.
	#[test]
	fn env_surface_matches_the_deploy_contract() {
		assert_eq!(Settings::var_names(), ["DATABASE_URL", "MIGRATE_DATABASE_URL", "PANEL_DATA_KEY", "SENTRY_DSN", "APP_ENV"]);
		assert_eq!(Settings::required_var_names("production"), ["DATABASE_URL", "PANEL_DATA_KEY"]);
		assert!(Settings::required_var_names("development").is_empty());
	}

	fn from(vars: &[(&str, &str)]) -> Result<Settings, ev_lib::settings::SettingsError> {
		let map: HashMap<String, String> = vars.iter().map(|(k, v)| ((*k).to_owned(), (*v).to_owned())).collect();
		Settings::from_source(|k| map.get(k).cloned())
	}

	#[test]
	fn secrets_are_lazy_and_never_print() {
		let s = from(&[]).unwrap();
		assert_eq!(format!("{:#}", s.data_key().unwrap_err()), "PANEL_DATA_KEY must be set");
		assert!(from(&[("PANEL_DATA_KEY", "short")]).unwrap().data_key().is_err());
		assert!(from(&[("APP_ENV", "production")]).is_err());
		let key = "ab".repeat(32);
		let s = from(&[("PANEL_DATA_KEY", &key), ("DATABASE_URL", "postgres://u:hunter2@db/panel")]).unwrap();
		assert!(s.data_key().is_ok());
		let shown = format!("{s:?}");
		assert!(!shown.contains(&key) && !shown.contains("hunter2"), "{shown}");
	}
}
