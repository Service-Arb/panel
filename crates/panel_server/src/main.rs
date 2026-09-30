//! Composition root: settings, error monitoring and logs, then the CLI over the engine's
//! `Panel`.

mod settings;

use std::{collections::BTreeSet, net::SocketAddr};

use clap::{Parser, Subcommand};
use ev_lib::error_monitoring;
use eyre::WrapErr;
use panel::{Panel, seal::DataKey, store::Store, telegram::Notifier};
use panel_core::{event::SourceKind, ids::BrandId};
use panel_server::{
	DEFAULT_BIND,
	concierge::Concierge,
	http,
	signin::{SignIn, SignInConfig},
	telegram::{self, BotApi, BotName},
	web::{self, Files},
};

use crate::settings::Settings;

#[derive(Parser)]
#[command(name = "panel", version, about = "The Service-Arb panel: sa.funnel.v1 ingest, the event journal and the funnel projections")]
struct Cli {
	#[command(subcommand)]
	cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
	/// Apply the migrations this build carries, as the schema's owner (MIGRATE_DATABASE_URL).
	/// Nothing else migrates: every other command refuses a database that lacks one.
	Migrate,
	/// The HTTP API.
	Serve {
		#[arg(long, default_value = DEFAULT_BIND)]
		bind: SocketAddr,
	},
	/// Rebuild leads, calls and payments from the journal, judging every event against the
	/// registry as it is now.
	RebuildProjections,
	/// The sources that may write events, and their keys.
	#[command(subcommand)]
	Source(SourceCmd),
	/// Print a fresh PANEL_DATA_KEY.
	GenDataKey,
}

#[derive(Subcommand)]
enum SourceCmd {
	/// Register a source; prints its secret, once.
	Add {
		/// Lowercase slug, e.g. aquafix-site.
		key_id: String,
		/// site | review_archive | gbp | posthog | panel | sheet | telephony
		#[arg(long)]
		kind: String,
		/// A brand it may write for; repeat for several.
		#[arg(long = "brand", required = true)]
		brands: Vec<String>,
	},
	List,
	/// Refuse its key from now on; what it wrote stays.
	Revoke {
		key_id: String,
	},
}

// Sentry must be initialised before the async runtime starts, hence no #[tokio::main].
fn main() -> eyre::Result<()> {
	color_eyre::install()?;

	// The deploy contract, straight out of the image: the gitops preflight diffs it with the
	// cluster Secret's keys, so a missing variable is caught before the rollout.
	if let Some(profile) = settings::print_required_vars_for() {
		for var in Settings::required_var_names(&profile) {
			println!("{var}");
		}
		return Ok(());
	}
	// Exits 78 (EX_CONFIG) on a bad environment, before anything else is built.
	let settings = ev_lib::settings::or_exit(Settings::from_env());

	// Held for the life of main: dropping it flushes. A no-op without SENTRY_DSN.
	let _sentry = error_monitoring::init(&error_monitoring::Config {
		dsn: settings.sentry_dsn.clone(),
		environment: settings.app_env.clone(),
		release: error_monitoring::release_name!().map(|r| r.into_owned()),
		// the same name OTEL uses, so an issue and its trace agree on the service
		service: std::env::var("OTEL_SERVICE_NAME").ok().filter(|s| !s.trim().is_empty()),
		traces_sample_rate: error_monitoring::Config::traces_sample_rate_for(&settings.app_env),
	});
	let cli = Cli::parse();
	init_tracing()?;

	tokio::runtime::Builder::new_multi_thread()
		.enable_all()
		.build()
		.wrap_err("building the tokio runtime")?
		.block_on(run(cli, settings))
}

/// Logs go to stderr, so a CLI command's stdout stays its output.
fn init_tracing() -> eyre::Result<()> {
	use tracing_subscriber::{EnvFilter, fmt, prelude::*};

	let filter = EnvFilter::try_from_default_env().or_else(|_| EnvFilter::try_new(option_env!("LOG_DIRECTIVES").unwrap_or("info")))?;
	tracing_subscriber::registry()
		.with(filter)
		.with(fmt::layer().with_writer(std::io::stderr))
		.with(error_monitoring::tracing_layer())
		.init();
	Ok(())
}

async fn run(cli: Cli, settings: Settings) -> eyre::Result<()> {
	let connect = || async { eyre::Ok(Panel::new(Store::connect(settings.database_url()?).await?, settings.data_key()?)) };
	match cli.cmd {
		Cmd::Migrate => {
			let options = settings.migrate_database_url()?.parse().wrap_err("MIGRATE_DATABASE_URL is not a Postgres URL")?;
			Store::migrate(options).await
		}
		Cmd::Serve { bind } => {
			let sign_in = settings.sign_in()?;
			let telegram = settings.telegram()?;
			let front_end = settings.web()?;
			serve(connect().await?, sign_in, telegram, front_end, bind).await
		}
		Cmd::RebuildProjections => {
			let r = connect().await?.rebuild_projections().await?;
			println!(
				"{} events: {} registered, {} unregistered, {} invalid; {} leads",
				r.events, r.registered, r.unregistered, r.invalid, r.leads
			);
			Ok(())
		}
		Cmd::Source(cmd) => source(&connect().await?, cmd).await,
		Cmd::GenDataKey => {
			println!("{}", DataKey::generate_hex()?);
			Ok(())
		}
	}
}

async fn source(panel: &Panel, cmd: SourceCmd) -> eyre::Result<()> {
	match cmd {
		SourceCmd::Add { key_id, kind, brands } => {
			let kind: SourceKind = kind.parse()?;
			let brands = brands.iter().map(|b| BrandId::parse(b)).collect::<Result<BTreeSet<_>, _>>()?;
			let Some(added) = panel.add_source(&key_id, kind, brands).await? else {
				eyre::bail!("a source {key_id} exists already");
			};
			println!("key id: {}\nsecret: {}\n(shown once; the source signs with it)", added.key_id, *added.secret);
			Ok(())
		}
		SourceCmd::List => {
			for s in panel.store().sources().await? {
				let brands: Vec<&str> = s.grant.brands.iter().map(BrandId::as_str).collect();
				let state = s.revoked_at.map_or_else(|| "active".to_owned(), |at| format!("revoked {at}"));
				println!("{:<24} {:<14} {:<32} {state}", s.grant.key_id, s.grant.kind, brands.join(","));
			}
			Ok(())
		}
		SourceCmd::Revoke { key_id } => {
			eyre::ensure!(panel.store().revoke_source(&key_id).await?, "no active source {key_id}");
			Ok(())
		}
	}
}

async fn serve(panel: Panel, sign_in: Option<settings::SignInSettings>, telegram: Option<settings::TelegramSettings>, front_end: Option<Files>, bind: SocketAddr) -> eyre::Result<()> {
	let (stop, stopped) = tokio::sync::watch::channel(false);
	let mut bot_work = None;
	let app = match sign_in {
		Some(s) => {
			let concierge = Concierge::new(&s.concierge_grpc, &s.client_secret)?;
			tracing::info!(panel_origin = s.panel_origin, "signing in through concierge");
			let bot = match telegram {
				Some(tg) => {
					let name = BotName::on(tg.username);
					let notifier = Notifier {
						panel: panel.clone(),
						bot: BotApi::new(telegram::API_BASE, &tg.token)?,
						concierge: concierge.clone(),
						locale: tg.locale,
					};
					tracing::info!("telegram notifications on");
					// Held, and awaited at shutdown: the poller gives its lease back on the way out.
					bot_work = Some(tokio::spawn(telegram::run(notifier, name.clone(), stopped)));
					name
				}
				None => {
					tracing::warn!("TELEGRAM_BOT_TOKEN unset: Telegram notifications off");
					BotName::off()
				}
			};
			http::app_with_telegram(
				SignIn::new(
					panel,
					concierge,
					SignInConfig {
						panel_origin: s.panel_origin,
						concierge_origin: s.concierge_origin,
					},
				),
				http::Limits::default(),
				bot,
			)
		}
		None => {
			tracing::warn!("sign-in not configured: serving ingest only, no /auth, no /api/v1");
			if telegram.is_some() {
				tracing::warn!("Telegram notifications off: they need the sign-in, to link users and confirm their access");
			}
			http::router(panel)
		}
	};
	let app = match front_end {
		Some(files) => web::serve(app, files),
		None => {
			tracing::warn!("PANEL_WEB_DIR unset: serving the API alone, no front end");
			app
		}
	};
	let listener = tokio::net::TcpListener::bind(bind).await.wrap_err_with(|| format!("binding {bind}"))?;
	tracing::info!(%bind, "serving");
	let served = axum::serve(listener, app).with_graceful_shutdown(shutdown_signal()).await.wrap_err("HTTP server");
	// Nobody may be listening any more: the bot is simply not running then.
	let _sent = stop.send(true);
	if let Some(work) = bot_work
		&& let Err(e) = work.await
	{
		panel_server::report(&eyre::eyre!(e), "the telegram worker panicked");
	}
	served
}

async fn shutdown_signal() {
	let ctrl_c = async {
		if let Err(e) = tokio::signal::ctrl_c().await {
			tracing::error!(error = %e, "listening for Ctrl-C");
			std::future::pending::<()>().await;
		}
	};
	#[cfg(unix)]
	let term = async {
		match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
			Ok(mut s) => {
				s.recv().await;
			}
			Err(e) => {
				tracing::error!(error = %e, "listening for SIGTERM");
				std::future::pending::<()>().await;
			}
		}
	};
	#[cfg(not(unix))]
	let term = std::future::pending::<()>();
	tokio::select! {
		() = ctrl_c => {}
		() = term => {}
	}
	tracing::info!("shutting down");
}
