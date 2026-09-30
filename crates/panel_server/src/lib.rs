//! The panel service: the HTTP surface over the engine's [`panel::Panel`] — ingest, the
//! sign-in through concierge and the operator API. The binary
//! (`panel`) adds the CLI and the process bootstrap.
//!
//! A library target too, so tests can serve the real router in-process.

pub mod api;
pub mod concierge;
pub mod cookies;
pub mod http;
pub mod signin;

/// Where `serve` listens unless told otherwise; the image binds the same port on 0.0.0.0.
pub const DEFAULT_BIND: &str = "127.0.0.1:59120";

/// Reports an error to Sentry and logs it — as a warning: the tracing layer would send an
/// error-level event to Sentry a second time.
pub fn report(e: &eyre::Report, what: &str) {
	ev_lib::error_monitoring::report(&**e);
	tracing::warn!(error = format!("{e:#}"), "{what}");
}
