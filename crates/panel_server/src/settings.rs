//! The environment: secrets and the deployment profile. Where to listen is a flag.

use panel::seal::DataKey;
use panel_core::notify::Locale;

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
		/// The panel's bot (`@evinvest_sa_bot`; not `telegram_token_main`). Unset: no Telegram
		/// notifications, and `serve` says so. Needs the sign-in configured too.
		#[secret]
		telegram_bot_token: Option<String>,
		/// The bot's username, for the `t.me/<bot>` links. Unset: asked of `getMe` at start.
		telegram_bot_username: Option<String>,
		/// The language of the bot's messages: `ru` or `en`.
		telegram_locale: String = "ru",
		/// The front end's static export (`frontend/` built, its `out/`), served on the same
		/// origin behind the API's routes; the image sets it. Unset: `serve` answers the API
		/// alone, and says so.
		panel_web_dir: Option<String>,
		/// PostHog's private API (the query API), not the capture host the landings send to
		/// (`us.i.posthog.com` answers no queries).
		posthog_api_host: String = "https://us.posthog.com",
		/// The PostHog project the landings send to (Service-Arb's). Unset, with the key below:
		/// no import, and `serve` says so.
		posthog_project_id: Option<String>,
		/// A personal API key with `query:read` on that project, nothing more.
		#[secret]
		posthog_personal_api_key: Option<String>,
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

/// The Telegram bot, when it is configured.
pub struct TelegramSettings {
	pub token: String,
	pub username: Option<String>,
	pub locale: Locale,
}

impl std::fmt::Debug for TelegramSettings {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("TelegramSettings")
			.field("username", &self.username)
			.field("locale", &self.locale)
			.finish_non_exhaustive()
	}
}

impl Settings {
	/// The front end's files, when `PANEL_WEB_DIR` names them.
	pub fn web(&self) -> eyre::Result<Option<panel_server::web::Files>> {
		let Some(dir) = self.panel_web_dir.as_deref().map(str::trim).filter(|d| !d.is_empty()) else {
			return Ok(None);
		};
		Ok(Some(panel_server::web::Files::new(std::path::Path::new(dir))?))
	}

	/// `None` without `TELEGRAM_BOT_TOKEN`: the bot is off.
	pub fn telegram(&self) -> eyre::Result<Option<TelegramSettings>> {
		let Some(token) = self.telegram_bot_token.as_deref().map(str::trim).filter(|t| !t.is_empty()) else {
			return Ok(None);
		};
		Ok(Some(TelegramSettings {
			token: token.to_owned(),
			username: self.telegram_bot_username.as_deref().map(str::trim).filter(|u| !u.is_empty()).map(str::to_owned),
			locale: self.telegram_locale.parse()?,
		}))
	}
}

/// The PostHog import, when it is configured.
pub struct PosthogSettings {
	pub api_host: String,
	pub project_id: String,
	pub api_key: String,
}

impl std::fmt::Debug for PosthogSettings {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("PosthogSettings")
			.field("api_host", &self.api_host)
			.field("project_id", &self.project_id)
			.finish_non_exhaustive()
	}
}

impl Settings {
	/// `None` without the project and the key: the import is off. One without the other is a
	/// mistake, named.
	pub fn posthog(&self) -> eyre::Result<Option<PosthogSettings>> {
		let set = |v: &Option<String>| v.as_deref().map(str::trim).filter(|v| !v.is_empty()).map(str::to_owned);
		let (project, key) = match (set(&self.posthog_project_id), set(&self.posthog_personal_api_key)) {
			(None, None) => return Ok(None),
			(Some(p), Some(k)) => (p, k),
			(Some(_), None) => eyre::bail!("the PostHog import needs POSTHOG_PERSONAL_API_KEY too"),
			(None, Some(_)) => eyre::bail!("the PostHog import needs POSTHOG_PROJECT_ID too"),
		};
		eyre::ensure!(
			!project.is_empty() && project.len() <= 32 && project.bytes().all(|b| b.is_ascii_digit()),
			"POSTHOG_PROJECT_ID is a number, e.g. 614067"
		);
		let host = self.posthog_api_host.trim().trim_end_matches('/');
		let url = url::Url::parse(host).map_err(|e| eyre::eyre!("POSTHOG_API_HOST is not a URL: {e}"))?;
		eyre::ensure!(url.scheme() == "https" || self.app_env != "production", "POSTHOG_API_HOST must be https in production");
		eyre::ensure!(url.path() == "/" && url.query().is_none(), "POSTHOG_API_HOST is an origin alone, e.g. https://us.posthog.com");
		Ok(Some(PosthogSettings {
			api_host: host.to_owned(),
			project_id: project,
			api_key: key,
		}))
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
		if self.app_env == "production" {
			bare_https_origin("PANEL_PUBLIC_ORIGIN", &get(&self.panel_public_origin))?;
			bare_https_origin("CONCIERGE_PUBLIC_ORIGIN", &get(&self.concierge_public_origin))?;
		}
		Ok(Some(SignInSettings {
			panel_origin: get(&self.panel_public_origin),
			concierge_origin: get(&self.concierge_public_origin),
			concierge_grpc: get(&self.concierge_grpc_addr),
			client_secret: get(&self.rp_client_secret_sa),
		}))
	}
}

/// In production an origin the browser is sent to or told about is `https://host[:port]`,
/// nothing after it: plain http would drop the `__Host-`/`Secure` cookies, and a path would
/// end up inside the redirect URI concierge compares byte for byte.
fn bare_https_origin(var: &str, raw: &str) -> eyre::Result<()> {
	let url = url::Url::parse(raw).map_err(|e| eyre::eyre!("{var} is not a URL: {e}"))?;
	eyre::ensure!(url.scheme() == "https", "{var} must be https in production");
	eyre::ensure!(url.host_str().is_some_and(|h| !h.is_empty()), "{var} has no host");
	eyre::ensure!(
		url.path() == "/" && url.query().is_none() && url.fragment().is_none() && url.username().is_empty() && url.password().is_none(),
		"{var} must be an origin alone, e.g. https://sa.evinvest.ltd"
	);
	Ok(())
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
				"TELEGRAM_BOT_TOKEN",
				"TELEGRAM_BOT_USERNAME",
				"TELEGRAM_LOCALE",
				"PANEL_WEB_DIR",
				"POSTHOG_API_HOST",
				"POSTHOG_PROJECT_ID",
				"POSTHOG_PERSONAL_API_KEY",
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

	#[test]
	fn telegram_is_off_without_a_token() {
		assert!(from(&[]).unwrap().telegram().unwrap().is_none());
		let token = "123456:secret-bot-token";
		let on = from(&[("TELEGRAM_BOT_TOKEN", token), ("TELEGRAM_BOT_USERNAME", "evinvest_sa_bot")]).unwrap();
		let tg = on.telegram().unwrap().unwrap();
		assert_eq!((tg.username.as_deref(), tg.locale), (Some("evinvest_sa_bot"), Locale::Ru));
		assert!(!format!("{on:?} {tg:?}").contains(token), "the token never prints");
		assert_eq!(
			from(&[("TELEGRAM_BOT_TOKEN", token), ("TELEGRAM_LOCALE", "en")]).unwrap().telegram().unwrap().unwrap().locale,
			Locale::En
		);
		assert!(from(&[("TELEGRAM_BOT_TOKEN", token), ("TELEGRAM_LOCALE", "de")]).unwrap().telegram().is_err());
	}

	#[test]
	fn the_posthog_import_is_off_without_its_key() {
		assert!(from(&[]).unwrap().posthog().unwrap().is_none());
		let key = "phx_secret";
		let on = from(&[("POSTHOG_PROJECT_ID", "614067"), ("POSTHOG_PERSONAL_API_KEY", key)]).unwrap();
		let ph = on.posthog().unwrap().unwrap();
		assert_eq!((ph.api_host.as_str(), ph.project_id.as_str()), ("https://us.posthog.com", "614067"));
		assert!(!format!("{on:?} {ph:?}").contains(key), "the key never prints");
		let half = from(&[("POSTHOG_PROJECT_ID", "614067")]).unwrap().posthog().unwrap_err();
		assert_eq!(format!("{half}"), "the PostHog import needs POSTHOG_PERSONAL_API_KEY too");
		assert!(from(&[("POSTHOG_PROJECT_ID", "../1"), ("POSTHOG_PERSONAL_API_KEY", key)]).unwrap().posthog().is_err());
		assert!(
			from(&[("POSTHOG_PROJECT_ID", "1"), ("POSTHOG_PERSONAL_API_KEY", key), ("POSTHOG_API_HOST", "https://us.posthog.com/api")])
				.unwrap()
				.posthog()
				.is_err()
		);
	}

	#[test]
	fn production_origins_are_bare_https() {
		let key = "0".repeat(64);
		let secret = "s".repeat(40);
		let with = |panel: &str, concierge: &str| {
			from(&[
				("APP_ENV", "production"),
				("DATABASE_URL", "postgres://localhost/x"),
				("PANEL_DATA_KEY", &key),
				("PANEL_PUBLIC_ORIGIN", panel),
				("CONCIERGE_PUBLIC_ORIGIN", concierge),
				("CONCIERGE_GRPC_ADDR", "http://concierge:55670"),
				("RP_CLIENT_SECRET_SA", &secret),
			])
			.unwrap()
			.sign_in()
		};
		assert!(with("https://sa.evinvest.ltd", "https://evinvest.ltd/").is_ok());
		for (panel, why) in [
			("http://sa.evinvest.ltd", "must be https"),
			("https://sa.evinvest.ltd/panel", "an origin alone"),
			("https://sa.evinvest.ltd/?a=1", "an origin alone"),
			("https://sa.evinvest.ltd/#x", "an origin alone"),
			("https://u:p@sa.evinvest.ltd", "an origin alone"),
			("sa.evinvest.ltd", "not a URL"),
		] {
			let e = format!("{}", with(panel, "https://evinvest.ltd").unwrap_err());
			assert!(e.contains("PANEL_PUBLIC_ORIGIN") && e.contains(why), "{panel}: {e}");
		}
		let e = format!("{}", with("https://sa.evinvest.ltd", "http://evinvest.ltd").unwrap_err());
		assert!(e.contains("CONCIERGE_PUBLIC_ORIGIN"), "{e}");
		assert!(
			from(&[
				("PANEL_PUBLIC_ORIGIN", "http://localhost:59120"),
				("CONCIERGE_PUBLIC_ORIGIN", "http://localhost:3000"),
				("CONCIERGE_GRPC_ADDR", "http://localhost:55670"),
				("RP_CLIENT_SECRET_SA", &secret),
			])
			.unwrap()
			.sign_in()
			.is_ok(),
			"plain http is for development"
		);
	}
}
