import { BOOKING_PROVIDERS, type BookingProvider, PAGE_PROVIDERS, type PageProvider } from "@/shared/config/booking";

/**
 * A place's `booking` setting: the providers it offers, each with its page,
 * and the one a visitor gets by default. `manual` is always offered and so is
 * never a key of `providers`.
 */
export interface BookingConfig {
  default: BookingProvider;
  providers: Partial<Record<PageProvider, { url: string }>>;
}

/** Why a URL is not a provider's booking page; the panel words each one. */
export type UrlProblem = "format" | "scheme" | "fragment" | "backslash" | "userinfo" | "ip" | "port" | "host" | "google" | "cal_host" | "cal_path";

/** Why a part of the setting is refused, beside the URL's own reasons. */
export type ConfigProblem = "shape" | "unknown_key" | "required" | "provider_unknown" | "provider_shape" | "provider_extra" | "manual" | "url_type" | "default_unknown" | "default_unset";

export type BookingProblem = UrlProblem | ConfigProblem;

/** The longest booking URL. */
export const MAX_BOOKING_URL = 2048;

/** kitstart's `calComHosts` default (`rules.json` of the fixtures). */
export const CAL_COM_HOSTS: readonly string[] = ["cal.evinvest.ltd", "cal.com"];

const LABEL = /^[A-Za-z0-9]([A-Za-z0-9-]{0,61}[A-Za-z0-9])?$/;
const CAL_SEGMENT = /^[A-Za-z0-9][A-Za-z0-9._-]{0,99}$/;
const PRINTABLE = /^[\x21-\x7e]+$/;
const APPOINTMENTS = "/calendar/appointments/";

const isPageProvider = (p: string): p is PageProvider => (PAGE_PROVIDERS as readonly string[]).includes(p);
const isProvider = (p: string): p is BookingProvider => (BOOKING_PROVIDERS as readonly string[]).includes(p);

/** The authority's own rules: no userinfo, no port, not a bracketed address, a dotted DNS name. */
function hostProblem(host: string): UrlProblem | null {
  if (host.includes("@")) return "userinfo";
  if (host.startsWith("[")) return "ip";
  if (host.includes(":")) return "port";
  const labels = host.split(".");
  if (host.length > 253 || labels.length < 2 || !labels.every((l) => LABEL.test(l))) return "host";
  const last = (labels.at(-1) ?? "").toLowerCase();
  // A WHATWG parser reads a host whose last label is a number as an IPv4 address.
  return /^\d+$/.test(last) || last.startsWith("0x") ? "ip" : null;
}

function providerProblem(provider: PageProvider, host: string, path: string): UrlProblem | null {
  switch (provider) {
    case "link":
      return null;
    case "google_calendar": {
      const short = host === "calendar.app.google" && path.length > 1;
      const long = host === "calendar.google.com" && path.length > APPOINTMENTS.length && path.startsWith(APPOINTMENTS);
      return short || long ? null : "google";
    }
    case "cal_com": {
      if (!CAL_COM_HOSTS.includes(host)) return "cal_host";
      const segments = path.replace(/^\//, "").split("/");
      return path.startsWith("/") && segments.length === 2 && segments.every((s) => CAL_SEGMENT.test(s)) ? null : "cal_path";
    }
  }
}

/**
 * Whether `url` may be `provider`'s booking page: the fixtures' URL rule
 * (normative), the same checks in the same order as `panel_core::booking::check_url`.
 */
export function checkBookingUrl(provider: PageProvider, url: string): UrlProblem | null {
  if (url.length === 0 || url.length > MAX_BOOKING_URL || !PRINTABLE.test(url)) return "format";
  if (!url.startsWith("https://")) return "scheme";
  if (url.includes("#")) return "fragment";
  if (url.includes("\\")) return "backslash";
  const rest = url.slice("https://".length);
  const split = rest.search(/[/?]/);
  const host = split < 0 ? rest : rest.slice(0, split);
  const tail = split < 0 ? "" : rest.slice(split);
  const bad = hostProblem(host);
  if (bad) return bad;
  return providerProblem(provider, host.toLowerCase(), tail.split("?")[0] ?? "");
}

const isObject = (v: unknown): v is Record<string, unknown> => typeof v === "object" && v !== null && !Array.isArray(v);

/** Each provider entry: known, an object of `url` alone, the URL good. Returns the providers that passed. */
function checkProviders(raw: unknown, problems: Map<string, BookingProblem>): PageProvider[] {
  if (!isObject(raw)) {
    problems.set("booking.providers", raw === undefined ? "required" : "shape");
    return [];
  }
  const named: PageProvider[] = [];
  for (const [name, conf] of Object.entries(raw)) {
    const path = `booking.providers.${name}`;
    if (name === "manual") problems.set(path, "manual");
    else if (!isPageProvider(name)) problems.set(path, "provider_unknown");
    else if (!isObject(conf)) problems.set(path, "provider_shape");
    else if (Object.keys(conf).some((k) => k !== "url")) problems.set(path, "provider_extra");
    else if (conf.url === undefined) problems.set(`${path}.url`, "required");
    else if (typeof conf.url !== "string") problems.set(`${path}.url`, "url_type");
    else {
      const bad = checkBookingUrl(name, conf.url);
      if (bad) problems.set(`${path}.url`, bad);
      else named.push(name);
    }
  }
  return named;
}

/**
 * A place's `booking` checked whole, as the server checks it before a 422:
 * every problem keyed by the 422's own path (`booking.default`,
 * `booking.providers.cal_com.url`). Empty means the server takes it.
 */
export function checkBookingConfig(v: unknown): Map<string, BookingProblem> {
  const problems = new Map<string, BookingProblem>();
  if (!isObject(v)) return problems.set("booking", "shape");
  if (Object.keys(v).some((k) => k !== "default" && k !== "providers")) problems.set("booking", "unknown_key");
  const raw = v.default;
  const def = typeof raw === "string" && isProvider(raw) ? raw : null;
  if (raw === undefined) problems.set("booking.default", "required");
  else if (def === null) problems.set("booking.default", "default_unknown");
  const named = checkProviders(v.providers, problems);
  // A default whose own entry is refused is blamed there, not twice.
  const blamed = [...problems.keys()].some((k) => def !== null && k.startsWith(`booking.providers.${def}`));
  if (def !== null && def !== "manual" && !named.includes(def) && !blamed) problems.set("booking.default", "default_unset");
  return problems;
}

/** The stored setting, when it is one the server would take; anything else is carried through untouched. */
export function bookingConfigOf(v: unknown): BookingConfig | null {
  if (checkBookingConfig(v).size > 0 || !isObject(v) || !isObject(v.providers)) return null;
  const def = BOOKING_PROVIDERS.find((p) => p === v.default);
  if (def === undefined) return null;
  const providers: BookingConfig["providers"] = {};
  for (const p of PAGE_PROVIDERS) {
    const conf = v.providers[p];
    if (isObject(conf) && typeof conf.url === "string") providers[p] = { url: conf.url };
  }
  return { default: def, providers };
}
