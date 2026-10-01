//! The front end: the Next static export (`frontend/`, `npm run build` → `out/`), served from
//! `PANEL_WEB_DIR` on the same origin as `/api` and `/auth`, behind every route of its own.
//!
//! ```text
//! /api, /auth, /health, /grafana   never the front end: a route of theirs, else a JSON 404
//! a directory                      its index.html (the export writes leads/index.html);
//!                                  /leads is redirected to /leads/ first
//! a file                           itself
//! anything else                    404.html, status 404
//! ```
//!
//! Files are read by tower-http's `ServeDir`, which refuses any path that would leave the
//! directory (`..`, a drive or a root component, encoded or not) before touching the disk.
//!
//! Headers: `/_next/static/*` is content-hashed, so `immutable` for a year; everything else
//! `no-cache`, and every HTML page a CSP.

use std::{io, path::Path};

use axum::{
	Json, Router,
	body::Body,
	extract::Request,
	http::{HeaderValue, StatusCode, header},
	response::{IntoResponse, Response},
};
use serde_json::json;
use tower_http::{
	services::{ServeDir, ServeFile},
	set_status::SetStatus,
};

/// Next inlines its bootstrap and the RSC payload as `<script>` elements, and the kit sets
/// inline styles, hence `'unsafe-inline'`; nothing is loaded from another origin (the fonts
/// are the system's, the kit's CSS is bundled), and the pages talk to this origin alone.
pub const CSP: &str = "default-src 'self'; script-src 'self' 'unsafe-inline'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; connect-src 'self'; frame-ancestors 'none'; base-uri 'none'; form-action 'self'";

/// Where the build puts what it names by content hash.
const HASHED: &str = "/_next/static/";

/// Paths that belong to the API side, whether or not a route of theirs is mounted: an unknown
/// one is a JSON 404, never the front end's page. `/grafana` is held for the dashboards.
const RESERVED: [&str; 4] = ["/api", "/auth", "/health", "/grafana"];

/// The export's files.
#[derive(Clone, Debug)]
pub struct Files {
	dir: ServeDir<SetStatus<ServeFile>>,
}

impl Files {
	/// The export in `dir`; it must hold `index.html` and `404.html`, so a wrong path fails the
	/// boot rather than every page.
	pub fn new(dir: &Path) -> eyre::Result<Self> {
		for page in ["index.html", "404.html"] {
			let path = dir.join(page);
			eyre::ensure!(path.is_file(), "PANEL_WEB_DIR {} has no {page}: not a build of frontend/", dir.display());
		}
		let dir = ServeDir::new(dir).append_index_html_on_directories(true).not_found_service(ServeFile::new(dir.join("404.html")));
		Ok(Self { dir })
	}
}

/// `app`, with the front end answering whatever none of its routes does.
pub fn serve(app: Router, files: Files) -> Router {
	app.fallback(move |req: Request| page(files.clone(), req))
}

fn reserved(path: &str) -> bool {
	RESERVED.iter().any(|r| path.strip_prefix(r).is_some_and(|rest| rest.is_empty() || rest.starts_with('/')))
}

async fn page(mut files: Files, mut req: Request) -> Response {
	let path = req.uri().path();
	if reserved(path) {
		return dressed((StatusCode::NOT_FOUND, Json(json!({ "error": "not found" }))).into_response(), false);
	}
	let hashed = path.starts_with(HASHED);
	// Every file in the image has the Nix store's mtime, 1970: answering If-Modified-Since
	// from it would keep a browser on the previous release's HTML, whose chunks are gone.
	// So nothing is revalidated by date — the HTML is small, and the rest is hashed.
	for h in [header::IF_MODIFIED_SINCE, header::IF_UNMODIFIED_SINCE] {
		req.headers_mut().remove(h);
	}
	let res = match files.dir.try_call(req).await {
		Ok(res) => res.map(Body::new),
		Err(e) => return failed(&e),
	};
	dressed(res, hashed)
}

fn failed(e: &io::Error) -> Response {
	crate::report(&eyre::eyre!("{e}"), "serving the front end");
	dressed(StatusCode::INTERNAL_SERVER_ERROR.into_response(), false)
}

fn dressed(mut res: Response, hashed: bool) -> Response {
	let ok = res.status().is_success();
	let h = res.headers_mut();
	h.remove(header::LAST_MODIFIED);
	h.insert(header::X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff"));
	h.insert(header::REFERRER_POLICY, HeaderValue::from_static("same-origin"));
	let html = h.get(header::CONTENT_TYPE).and_then(|v| v.to_str().ok()).is_some_and(|v| v.starts_with("text/html"));
	let cache = if hashed && ok && !html { "public, max-age=31536000, immutable" } else { "no-cache" };
	h.insert(header::CACHE_CONTROL, HeaderValue::from_static(cache));
	if html {
		h.insert(header::CONTENT_SECURITY_POLICY, HeaderValue::from_static(CSP));
	}
	res
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn reserved_prefixes_are_whole_segments() {
		for p in ["/api", "/api/", "/api/v1/nope", "/auth/x", "/health", "/grafana", "/grafana/d/1"] {
			assert!(reserved(p), "{p}");
		}
		for p in ["/", "/apiary/", "/authors", "/healthy", "/leads/", "/_next/static/a.js"] {
			assert!(!reserved(p), "{p}");
		}
	}
}
