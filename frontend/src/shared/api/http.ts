import { type Parser, parse } from "@/shared/lib/parse";

import { CSRF_HEADER, readCsrf } from "./csrf";

/**
 * Why a call failed, in the words the screens act on. The backend's gate
 * (`panel_server::signin::gate`) answers 401 for no session, 403 with
 * `"no access to the panel"` for a signed-in person without a grant, 403 `"csrf"`
 * for a write without the header, and 503 while concierge is unreachable —
 * which keeps the session, so the front end must not sign anyone out on it.
 */
export type ApiFailure =
  | { kind: "unauthenticated" }
  | { kind: "no_access" }
  | { kind: "csrf" }
  | { kind: "forbidden"; message: string }
  | { kind: "unavailable" }
  | { kind: "not_found" }
  | { kind: "conflict"; message: string }
  | { kind: "bad_request"; message: string }
  | { kind: "failed"; status: number; message: string }
  | { kind: "network" };

export class ApiError extends Error {
  override name = "ApiError";
  constructor(readonly failure: ApiFailure) {
    super(`api: ${failure.kind}`);
  }
}

export interface HttpDeps {
  fetch: typeof fetch;
  /** The page's cookies, as `document.cookie` has them. */
  cookie: () => string;
  /** A 401: the session is gone. The browser goes to the backend's sign-in. */
  onUnauthenticated: () => void;
}

export type Query = Record<string, string | number | boolean | null | undefined>;

function withQuery(path: string, query?: Query): string {
  if (!query) return path;
  const qs = new URLSearchParams();
  for (const [k, v] of Object.entries(query)) if (v !== undefined && v !== null && v !== "") qs.set(k, String(v));
  const s = qs.toString();
  return s ? `${path}?${s}` : path;
}

async function errorMessage(res: Response): Promise<string> {
  try {
    const body: unknown = await res.json();
    if (typeof body === "object" && body !== null && "error" in body && typeof body.error === "string") return body.error;
  } catch {
    // Not JSON: a proxy's page, say. The status says enough.
  }
  return "";
}

export async function failureOf(res: Response): Promise<ApiFailure> {
  const message = await errorMessage(res);
  switch (res.status) {
    case 401:
      return { kind: "unauthenticated" };
    case 403:
      if (message === "csrf") return { kind: "csrf" };
      if (message === "no access to the panel") return { kind: "no_access" };
      return { kind: "forbidden", message };
    case 404:
      return { kind: "not_found" };
    case 409:
      return { kind: "conflict", message };
    case 400:
      return { kind: "bad_request", message };
    case 503:
      return { kind: "unavailable" };
    default:
      return { kind: "failed", status: res.status, message };
  }
}

export interface Http {
  get<T>(path: string, parser: Parser<T>, query?: Query): Promise<T>;
  /** Every write carries the CSRF header; the backend refuses one without it. */
  send<T>(method: "POST" | "DELETE", path: string, body: unknown, parser: Parser<T>): Promise<T>;
}

export function createHttp(deps: HttpDeps): Http {
  async function call<T>(path: string, init: RequestInit, parser: Parser<T>): Promise<T> {
    let res: Response;
    try {
      res = await deps.fetch(path, { credentials: "same-origin", cache: "no-store", ...init });
    } catch {
      throw new ApiError({ kind: "network" });
    }
    if (!res.ok) {
      const failure = await failureOf(res);
      if (failure.kind === "unauthenticated") deps.onUnauthenticated();
      throw new ApiError(failure);
    }
    const body: unknown = res.status === 204 ? null : await res.json();
    return parse(parser, body);
  }

  return {
    get: (path, parser, query) => call(withQuery(path, query), { method: "GET", headers: { accept: "application/json" } }, parser),
    send: (method, path, body, parser) => {
      const headers: Record<string, string> = { accept: "application/json" };
      const csrf = readCsrf(deps.cookie());
      if (csrf) headers[CSRF_HEADER] = csrf;
      const init: RequestInit = { method, headers };
      if (body !== undefined) {
        headers["content-type"] = "application/json";
        init.body = JSON.stringify(body);
      }
      return call(path, init, parser);
    },
  };
}

/** The backend's sign-in; a full navigation, since it answers with a redirect to concierge. */
export const SIGN_IN_PATH = "/auth/login";

export const http: Http = createHttp({
  fetch: (input, init) => fetch(input, init),
  cookie: () => (typeof document === "undefined" ? "" : document.cookie),
  onUnauthenticated: () => {
    // /auth/login is the Rust backend's route, not a page of this app: only a full navigation reaches it.
    // eslint-disable-next-line @next/next/no-location-assign-relative-destination
    if (typeof window !== "undefined") window.location.assign(SIGN_IN_PATH);
  },
});

/** For bodies the screens do not read (`201 {event_id}`, `204`). */
export const ignoreBody: Parser<null> = () => null;
