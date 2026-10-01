//! The PostHog import's adapter and schedule: HogQL over PostHog's query API (reqwest), and
//! the background pass of `serve` that runs [`panel::Panel::import_posthog`] once an hour on
//! one replica. What is counted and how it is journaled is the engine's (`panel::posthog`).
//!
//! Outbound only: the pods need egress to the API host (`POSTHOG_API_HOST`, `:443`). Without
//! `POSTHOG_PROJECT_ID` and `POSTHOG_PERSONAL_API_KEY` none of this runs, and `serve` says so.

use std::{sync::Arc, time::Duration};

use eyre::WrapErr;
use jiff::Timestamp;
use panel::{
	Panel,
	posthog::{Hogql, Imported, WINDOW_DAYS},
};
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::sync::watch;
use uuid::Uuid;
use zeroize::Zeroizing;

/// How often a replica asks whether an import is due; the lease makes it once an hour.
const TICK: Duration = Duration::from_secs(5 * 60);

/// PostHog's query API for one project, with a personal API key (`query:read`).
#[derive(Clone)]
pub struct QueryApi {
	http: reqwest::Client,
	url: String,
	project_id: String,
	key: Arc<Zeroizing<String>>,
}

impl std::fmt::Debug for QueryApi {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("QueryApi").field("url", &self.url).finish_non_exhaustive()
	}
}

#[derive(Deserialize)]
struct Answer {
	results: Option<Vec<Vec<Value>>>,
}

impl QueryApi {
	/// `host`: the API's origin, e.g. `https://us.posthog.com`, or a test's mock.
	pub fn new(host: &str, project_id: &str, key: &str) -> eyre::Result<Self> {
		let http = reqwest::Client::builder()
			.connect_timeout(Duration::from_secs(5))
			.timeout(Duration::from_secs(60))
			.redirect(reqwest::redirect::Policy::none())
			.build()?;
		Ok(Self {
			http,
			url: format!("{}/api/projects/{project_id}/query/", host.trim_end_matches('/')),
			project_id: project_id.to_owned(),
			key: Arc::new(Zeroizing::new(key.to_owned())),
		})
	}

	/// What the journal calls the import: `posthog-<project>`.
	pub fn source_id(&self) -> String {
		format!("posthog-{}", self.project_id)
	}
}

impl Hogql for QueryApi {
	async fn query(&self, hogql: &str, values: &Value) -> eyre::Result<Vec<Vec<Value>>> {
		let body = json!({"query": {"kind": "HogQLQuery", "query": hogql, "values": values}});
		let res = self
			.http
			.post(&self.url)
			.bearer_auth(self.key.as_str())
			.json(&body)
			.send()
			.await
			.map_err(|e| eyre::eyre!("PostHog did not answer: {e}"))?;
		let status = res.status();
		if !status.is_success() {
			// PostHog's own words about the query, cut short; the key is never in them.
			let why: String = res.text().await.unwrap_or_default().chars().take(300).collect();
			eyre::bail!("PostHog answered {status}: {why}");
		}
		let answer: Answer = res.json().await.wrap_err("PostHog's answer is not the query API's JSON")?;
		answer.results.ok_or_else(|| eyre::eyre!("PostHog's answer has no results table"))
	}
}

/// Runs the import until `shutdown` turns true: every five minutes, the replica that gets the
/// lease (when an import is due) counts the last [`WINDOW_DAYS`] days.
pub async fn run(panel: Panel, api: QueryApi, shutdown: watch::Receiver<bool>) {
	let holder = Uuid::now_v7();
	let source = api.source_id();
	crate::every(TICK, shutdown, "the posthog import", || async {
		once(&panel, &api, &source, holder, WINDOW_DAYS, false).await.map(drop)
	})
	.await;
}

/// One import of the last `days` days under the lease: `None` when another replica holds it
/// or none is due. `force`: whenever no one holds it (the CLI's one-off), else an error.
pub async fn once(panel: &Panel, api: &impl Hogql, source: &str, holder: Uuid, days: u16, force: bool) -> eyre::Result<Option<Imported>> {
	if !panel.posthog_import_lease(holder, Timestamp::now(), force).await? {
		eyre::ensure!(!force, "another replica is importing; try again in a few minutes");
		return Ok(None);
	}
	let done = panel.import_posthog(api, source, days, Timestamp::now()).await;
	panel.posthog_import_release(holder, Timestamp::now(), done.is_ok()).await?;
	done.map(Some)
}
