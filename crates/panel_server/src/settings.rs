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
		/// Where browsers reach the panel, e.g. `https://sa.evinvest.ltd`. The sign-in's
		/// redirect_uri is `<this>/auth/callback`, registered at concierge byte for byte; an
		/// `https` origin makes the cookies `__Host-` and `Secure`.
		#[required_in("production")]
		panel_public_origin: Option<String>,
		/// Where browsers reach concierge's `/api/auth/authorize`, e.g. `https://evinvest.ltd`.
		#[required_in("production")]
		concierge_public_origin: Option<String>,
		/// concierge's gRPC, from inside the cluster, e.g. `http://concierge:55670`.
		#[required_in("production")]
		concierge_grpc_addr: Option<String>,
		/// The panel's client secret at concierge (client id `sa`); concierge holds its hash.
		#[secret]
		#[required_in("production")]
		rp_client_secret_sa: Option<String>,
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

/// What signing in needs, all of it or none.
pub struct SignInSettings {
	pub panel_origin: String,
	pub concierge_origin: String,
	pub concierge_grpc: String,
	pub client_secret: String,
}

impl std::fmt::Debug for SignInSettings {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("SignInSettings")
			.field("panel_origin", &self.panel_origin)
			.field("concierge_origin", &self.concierge_origin)
			.field("concierge_grpc", &self.concierge_grpc)
			.finish_non_exhaustive()
	}
}

impl Settings {
	/// `None` when none of the sign-in variables is set: `serve` then answers ingest alone.
	/// Some but not all is a mistake, named.
	pub fn sign_in(&self) -> eyre::Result<Option<SignInSettings>> {
		let vars = [
			("PANEL_PUBLIC_ORIGIN", &self.panel_public_origin),
			("CONCIERGE_PUBLIC_ORIGIN", &self.concierge_public_origin),
			("CONCIERGE_GRPC_ADDR", &self.concierge_grpc_addr),
			("RP_CLIENT_SECRET_SA", &self.rp_client_secret_sa),
		];
		let set = |v: &Option<String>| v.as_deref().map(str::trim).filter(|v| !v.is_empty()).map(str::to_owned);
		let missing: Vec<&str> = vars.iter().filter(|(_, v)| set(v).is_none()).map(|(name, _)| *name).collect();
		if missing.len() == vars.len() {
			return Ok(None);
		}
		eyre::ensure!(missing.is_empty(), "signing in needs {} too", missing.join(", "));
		let get = |v: &Option<String>| set(v).unwrap_or_default();
		Ok(Some(SignInSettings {
			panel_origin: get(&self.panel_public_origin),
			concierge_origin: get(&self.concierge_public_origin),
			concierge_grpc: get(&self.concierge_grpc_addr),
			client_secret: get(&self.rp_client_secret_sa),
		}))
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
		assert_eq!(
			Settings::var_names(),
			[
				"DATABASE_URL",
				"MIGRATE_DATABASE_URL",
				"PANEL_DATA_KEY",
				"SENTRY_DSN",
				"PANEL_PUBLIC_ORIGIN",
				"CONCIERGE_PUBLIC_ORIGIN",
				"CONCIERGE_GRPC_ADDR",
				"RP_CLIENT_SECRET_SA",
				"APP_ENV"
			]
		);
		assert_eq!(
			Settings::required_var_names("production"),
			[
				"DATABASE_URL",
				"PANEL_DATA_KEY",
				"PANEL_PUBLIC_ORIGIN",
				"CONCIERGE_PUBLIC_ORIGIN",
				"CONCIERGE_GRPC_ADDR",
				"RP_CLIENT_SECRET_SA"
			]
		);
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

	#[test]
	fn sign_in_is_all_or_nothing() {
		assert!(from(&[]).unwrap().sign_in().unwrap().is_none());
		let partial = from(&[("PANEL_PUBLIC_ORIGIN", "https://sa.evinvest.ltd")]).unwrap().sign_in().unwrap_err();
		assert_eq!(format!("{partial}"), "signing in needs CONCIERGE_PUBLIC_ORIGIN, CONCIERGE_GRPC_ADDR, RP_CLIENT_SECRET_SA too");
		let secret = "s".repeat(40);
		let full = from(&[
			("PANEL_PUBLIC_ORIGIN", "https://sa.evinvest.ltd"),
			("CONCIERGE_PUBLIC_ORIGIN", "https://evinvest.ltd"),
			("CONCIERGE_GRPC_ADDR", "http://concierge:55670"),
			("RP_CLIENT_SECRET_SA", &secret),
		])
		.unwrap();
		assert_eq!(full.sign_in().unwrap().unwrap().concierge_grpc, "http://concierge:55670");
		assert!(!format!("{full:?}").contains(&secret), "the client secret never prints");
	}
}
