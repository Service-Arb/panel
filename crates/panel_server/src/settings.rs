//! The environment: secrets and the deployment profile. Where to listen is a flag.

use panel::seal::DataKey;
use panel_core::{notify::Locale, role::Role};

ev_lib::settings! {
	/// Each secret is needed only by what uses it; a missing one fails that, with an error
	/// naming it, rather than the boot — except in production, where the only thing this
	/// binary runs is `serve`, which needs both.
	pub struct Settings {
		/// The panel's SQLite file, e.g. `/data/panel.db` (the image's, on the pod's volume,
		/// replicated by litestream). Created if missing and migrated on open, by whichever
		/// command opens it. A path, not a secret.
		#[required_in("production")]
		panel_db_path: Option<String>,
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
		/// The panel's Google OAuth client (Desktop kind), for the `google_calendar` booking
		/// adapter and `panel booking google-authorize`. Unset, with the secret below: no
		/// Google Calendar pull. Each brand's calendar is `GOOGLE_CALENDAR_REFRESH_TOKEN_<BRAND>`
		/// (secret) and `GOOGLE_CALENDAR_ID_<BRAND>` (optional, `primary`), read apart: their
		/// names follow the brands.
		google_oauth_client_id: Option<String>,
		#[secret]
		google_oauth_client_secret: Option<String>,
		/// How often each brand's calendar is pulled, in minutes.
		google_calendar_sync_minutes: u32 = "5",
		/// Development only: `admin` or `operator`. Signs whoever opens `/auth/login` in as a
		/// made-up user of that role, without concierge. Refused at start in any profile but
		/// `development`, beside any concierge variable, and unless PANEL_PUBLIC_ORIGIN is
		/// `http://localhost[:port]` or `http://127.0.0.1[:port]`.
		panel_dev_sign_in: Option<String>,
		/// The dev user's email; `dev-<role>@localhost` by default.
		panel_dev_sign_in_email: Option<String>,
		app_env: String = "development",
	}
}

impl Settings {
	pub fn db_path(&self) -> eyre::Result<&std::path::Path> {
		let path = self.panel_db_path.as_deref().map(str::trim).filter(|p| !p.is_empty());
		Ok(std::path::Path::new(path.ok_or_else(|| eyre::eyre!("PANEL_DB_PATH must be set"))?))
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

/// The prefix of a brand's Google Calendar refresh token: `…_VIFNET`.
pub const GOOGLE_REFRESH_PREFIX: &str = "GOOGLE_CALENDAR_REFRESH_TOKEN_";
/// The prefix of a brand's calendar id.
pub const GOOGLE_CALENDAR_ID_PREFIX: &str = "GOOGLE_CALENDAR_ID_";

/// The Google Calendar adapter, when it is configured.
pub struct GoogleSettings {
	pub client_id: String,
	pub client_secret: String,
	pub every_minutes: u32,
	/// Each brand with a refresh token, and its calendar.
	pub calendars: std::collections::BTreeMap<panel_core::ids::BrandId, panel_server::google_calendar::BrandCalendar>,
}

impl std::fmt::Debug for GoogleSettings {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("GoogleSettings")
			.field("client_id", &self.client_id)
			.field("every_minutes", &self.every_minutes)
			.field("calendars", &self.calendars)
			.finish_non_exhaustive()
	}
}

/// A brand from the end of a variable's name: `VIFNET` → `vifnet`. A brand whose id has a `-`
/// cannot be named so (none has).
fn brand_of_suffix(suffix: &str) -> eyre::Result<panel_core::ids::BrandId> {
	panel_core::ids::BrandId::parse(&suffix.to_ascii_lowercase()).map_err(|_| eyre::eyre!("{GOOGLE_REFRESH_PREFIX}{suffix}: {suffix} does not name a brand"))
}

impl Settings {
	/// The OAuth client alone, for the CLI's consent: both or neither.
	pub fn google_client(&self) -> eyre::Result<Option<(String, String)>> {
		let set = |v: &Option<String>| v.as_deref().map(str::trim).filter(|v| !v.is_empty()).map(str::to_owned);
		match (set(&self.google_oauth_client_id), set(&self.google_oauth_client_secret)) {
			(None, None) => Ok(None),
			(Some(id), Some(secret)) => Ok(Some((id, secret))),
			(Some(_), None) => eyre::bail!("GOOGLE_OAUTH_CLIENT_ID needs GOOGLE_OAUTH_CLIENT_SECRET too"),
			(None, Some(_)) => eyre::bail!("GOOGLE_OAUTH_CLIENT_SECRET needs GOOGLE_OAUTH_CLIENT_ID too"),
		}
	}

	/// `None` without the OAuth client or without any brand's token: the pull is off. `vars`:
	/// the environment's variables (the brands' are read from their names).
	pub fn google(&self, vars: impl IntoIterator<Item = (String, String)>) -> eyre::Result<Option<GoogleSettings>> {
		let vars: Vec<(String, String)> = vars.into_iter().filter(|(_, v)| !v.trim().is_empty()).collect();
		let mut calendars = std::collections::BTreeMap::new();
		for (name, token) in &vars {
			let Some(suffix) = name.strip_prefix(GOOGLE_REFRESH_PREFIX) else { continue };
			let brand = brand_of_suffix(suffix)?;
			let id_var = format!("{GOOGLE_CALENDAR_ID_PREFIX}{suffix}");
			let calendar_id = vars.iter().find(|(k, _)| *k == id_var).map_or_else(|| "primary".to_owned(), |(_, v)| v.trim().to_owned());
			calendars.insert(
				brand,
				panel_server::google_calendar::BrandCalendar {
					refresh_token: zeroize::Zeroizing::new(token.trim().to_owned()),
					calendar_id,
				},
			);
		}
		let client = self.google_client()?;
		match (client, calendars.is_empty()) {
			(None, true) => Ok(None),
			(None, false) => eyre::bail!("a {GOOGLE_REFRESH_PREFIX}<BRAND> is set: GOOGLE_OAUTH_CLIENT_ID and GOOGLE_OAUTH_CLIENT_SECRET are needed too"),
			(Some(_), true) => Ok(None),
			(Some((client_id, client_secret)), false) => {
				eyre::ensure!(self.google_calendar_sync_minutes >= 1, "GOOGLE_CALENDAR_SYNC_MINUTES is 1 or more");
				Ok(Some(GoogleSettings {
					client_id,
					client_secret,
					every_minutes: self.google_calendar_sync_minutes,
					calendars,
				}))
			}
		}
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

/// `PANEL_DEV_SIGN_IN`, checked: who everyone signs in as, and where.
#[derive(Debug)]
pub struct DevSignIn {
	pub role: Role,
	pub email: String,
	pub panel_origin: String,
}

impl Settings {
	/// `None` without `PANEL_DEV_SIGN_IN`. With it, refused unless this is development on the
	/// developer's own machine — the way concierge refuses `RP_DEV_REDIRECT_URIS`: anyone who
	/// reaches `/auth/login` is let in, so it must never be reachable from anywhere else.
	pub fn dev_sign_in(&self) -> eyre::Result<Option<DevSignIn>> {
		let set = |v: &Option<String>| v.as_deref().map(str::trim).filter(|v| !v.is_empty()).map(str::to_owned);
		let Some(role) = set(&self.panel_dev_sign_in) else {
			return Ok(None);
		};
		eyre::ensure!(
			self.app_env == ev_lib::settings::DEFAULT_PROFILE,
			"PANEL_DEV_SIGN_IN must never be set outside development (APP_ENV={}): it signs anyone in without concierge",
			self.app_env
		);
		let concierge: Vec<&str> = [
			("CONCIERGE_PUBLIC_ORIGIN", &self.concierge_public_origin),
			("CONCIERGE_GRPC_ADDR", &self.concierge_grpc_addr),
			("RP_CLIENT_SECRET_SA", &self.rp_client_secret_sa),
		]
		.into_iter()
		.filter(|(_, v)| set(v).is_some())
		.map(|(name, _)| name)
		.collect();
		eyre::ensure!(concierge.is_empty(), "PANEL_DEV_SIGN_IN replaces the concierge sign-in: unset {}", concierge.join(", "));
		let role: Role = role.parse().map_err(|e| eyre::eyre!("PANEL_DEV_SIGN_IN: {e}"))?;
		let origin = set(&self.panel_public_origin).ok_or_else(|| eyre::eyre!("PANEL_DEV_SIGN_IN needs PANEL_PUBLIC_ORIGIN, e.g. http://127.0.0.1:59120"))?;
		loopback_http_origin(&origin)?;
		let email = set(&self.panel_dev_sign_in_email).unwrap_or_else(|| format!("dev-{}@localhost", role.as_str()));
		Ok(Some(DevSignIn { role, email, panel_origin: origin }))
	}
}

/// `http://localhost[:port]` or `http://127.0.0.1[:port]`, nothing after it: an origin only
/// this machine's browser reaches.
fn loopback_http_origin(raw: &str) -> eyre::Result<()> {
	let refused = || eyre::eyre!("PANEL_DEV_SIGN_IN: PANEL_PUBLIC_ORIGIN must be http://localhost[:port] or http://127.0.0.1[:port], not {raw:?}");
	let url = url::Url::parse(raw).map_err(|_| refused())?;
	let loopback = matches!(url.host(), Some(url::Host::Domain("localhost") | url::Host::Ipv4(std::net::Ipv4Addr::LOCALHOST)));
	let bare = url.path() == "/" && url.query().is_none() && url.fragment().is_none() && url.username().is_empty() && url.password().is_none();
	eyre::ensure!(url.scheme() == "http" && loopback && bare, refused());
	Ok(())
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
				"PANEL_DB_PATH",
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
				"GOOGLE_OAUTH_CLIENT_ID",
				"GOOGLE_OAUTH_CLIENT_SECRET",
				"GOOGLE_CALENDAR_SYNC_MINUTES",
				"PANEL_DEV_SIGN_IN",
				"PANEL_DEV_SIGN_IN_EMAIL",
				"APP_ENV"
			]
		);
		assert_eq!(
			Settings::required_var_names("production"),
			[
				"PANEL_DB_PATH",
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
		let s = from(&[("PANEL_DATA_KEY", &key), ("PANEL_DB_PATH", "/data/panel.db")]).unwrap();
		assert!(s.data_key().is_ok());
		assert_eq!(s.db_path().unwrap(), std::path::Path::new("/data/panel.db"));
		assert_eq!(format!("{}", from(&[]).unwrap().db_path().unwrap_err()), "PANEL_DB_PATH must be set");
		let shown = format!("{s:?}");
		assert!(!shown.contains(&key), "{shown}");
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
	fn google_calendar_is_off_without_a_client_or_a_token() {
		let vars = |v: &[(&str, &str)]| v.iter().map(|(k, v)| ((*k).to_owned(), (*v).to_owned())).collect::<Vec<_>>();
		let token = "1//refresh-secret";
		assert!(from(&[]).unwrap().google(vars(&[])).unwrap().is_none());
		let client = from(&[("GOOGLE_OAUTH_CLIENT_ID", "cid"), ("GOOGLE_OAUTH_CLIENT_SECRET", "csecret")]).unwrap();
		assert!(client.google(vars(&[])).unwrap().is_none(), "no brand: off");
		let on = client
			.google(vars(&[
				("GOOGLE_CALENDAR_REFRESH_TOKEN_VIFNET", token),
				("GOOGLE_CALENDAR_ID_VIFNET", "bookings@group.calendar.google.com"),
				("GOOGLE_CALENDAR_REFRESH_TOKEN_AQUAFIX", token),
			]))
			.unwrap()
			.unwrap();
		let brands: Vec<&str> = on.calendars.keys().map(|b| b.as_str()).collect();
		assert_eq!(brands, ["aquafix", "vifnet"]);
		assert_eq!(
			on.calendars.values().map(|c| c.calendar_id.as_str()).collect::<Vec<_>>(),
			["primary", "bookings@group.calendar.google.com"]
		);
		assert_eq!(on.every_minutes, 5);
		assert!(!format!("{on:?} {client:?}").contains(token) && !format!("{client:?}").contains("csecret"), "secrets never print");
		let e = from(&[]).unwrap().google(vars(&[("GOOGLE_CALENDAR_REFRESH_TOKEN_VIFNET", token)])).unwrap_err();
		assert!(format!("{e}").contains("GOOGLE_OAUTH_CLIENT_ID"), "{e}");
		assert!(from(&[("GOOGLE_OAUTH_CLIENT_ID", "cid")]).unwrap().google_client().is_err());
		assert!(client.google(vars(&[("GOOGLE_CALENDAR_REFRESH_TOKEN_VIF NET", token)])).is_err());
	}

	#[test]
	fn production_origins_are_bare_https() {
		let key = "0".repeat(64);
		let secret = "s".repeat(40);
		let with = |panel: &str, concierge: &str| {
			from(&[
				("APP_ENV", "production"),
				("PANEL_DB_PATH", "/data/panel.db"),
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

	#[test]
	fn dev_sign_in_is_development_on_loopback_only() {
		assert!(from(&[]).unwrap().dev_sign_in().unwrap().is_none());
		let dev = |vars: &[(&str, &str)]| from(vars).unwrap().dev_sign_in();
		let on = dev(&[("PANEL_DEV_SIGN_IN", "admin"), ("PANEL_PUBLIC_ORIGIN", "http://127.0.0.1:59120")]).unwrap().unwrap();
		assert_eq!((on.role, on.email.as_str()), (Role::Admin, "dev-admin@localhost"));
		let op = dev(&[
			("PANEL_DEV_SIGN_IN", "operator"),
			("PANEL_DEV_SIGN_IN_EMAIL", "ann@example.com"),
			("PANEL_PUBLIC_ORIGIN", "http://localhost:3120"),
		])
		.unwrap()
		.unwrap();
		assert_eq!((op.role, op.email.as_str()), (Role::Operator, "ann@example.com"));
		assert!(dev(&[("PANEL_DEV_SIGN_IN", "admin"), ("PANEL_PUBLIC_ORIGIN", "http://localhost")]).is_ok(), "port optional");

		for origin in [
			"https://sa.evinvest.ltd",
			"http://sa.evinvest.ltd",
			"https://localhost:59120",
			"http://localhost.evil.example:59120",
			"http://127.0.0.1.nip.io:59120",
			"http://10.0.0.5:59120",
			"http://0.0.0.0:59120",
			"http://127.0.0.1:59120/panel",
			"http://u:p@127.0.0.1:59120",
			"127.0.0.1:59120",
		] {
			let e = format!("{}", dev(&[("PANEL_DEV_SIGN_IN", "admin"), ("PANEL_PUBLIC_ORIGIN", origin)]).unwrap_err());
			assert!(e.contains("PANEL_PUBLIC_ORIGIN must be http://localhost"), "{origin}: {e}");
		}
		let e = format!("{}", dev(&[("PANEL_DEV_SIGN_IN", "admin")]).unwrap_err());
		assert!(e.contains("needs PANEL_PUBLIC_ORIGIN"), "{e}");
		let e = format!("{}", dev(&[("PANEL_DEV_SIGN_IN", "owner"), ("PANEL_PUBLIC_ORIGIN", "http://127.0.0.1:59120")]).unwrap_err());
		assert!(e.contains("not one of operator, admin"), "{e}");
		let e = format!(
			"{}",
			dev(&[
				("PANEL_DEV_SIGN_IN", "admin"),
				("PANEL_PUBLIC_ORIGIN", "http://127.0.0.1:59120"),
				("CONCIERGE_GRPC_ADDR", "http://localhost:55670")
			])
			.unwrap_err()
		);
		assert!(e.contains("unset CONCIERGE_GRPC_ADDR"), "{e}");
	}

	#[test]
	fn dev_sign_in_is_refused_in_production() {
		let key = "0".repeat(64);
		let secret = "s".repeat(40);
		let prod = |profile: &str| {
			from(&[
				("APP_ENV", profile),
				("PANEL_DB_PATH", "/data/panel.db"),
				("PANEL_DATA_KEY", &key),
				("PANEL_PUBLIC_ORIGIN", "http://127.0.0.1:59120"),
				("CONCIERGE_PUBLIC_ORIGIN", "https://evinvest.ltd"),
				("CONCIERGE_GRPC_ADDR", "http://concierge:55670"),
				("RP_CLIENT_SECRET_SA", &secret),
				("PANEL_DEV_SIGN_IN", "admin"),
			])
			.unwrap()
			.dev_sign_in()
		};
		for profile in ["production", "staging"] {
			let e = format!("{}", prod(profile).unwrap_err());
			assert!(e.contains("must never be set outside development"), "{profile}: {e}");
		}
	}
}
