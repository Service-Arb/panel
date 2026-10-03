/**
 * Experiments as config for the dev stub: `GET /api/v1/experiments` and the
 * admin's `PUT /api/v1/experiments/{brand}/{key}`, in memory. aquafix declares
 * two (one with a holdout), vifnet one, and a retired one stays listed.
 *
 *   STUB_POSTHOG=off   no PostHog project: every `posthog_url` is null
 *
 * The 400 checks are the landing's rules (`applyOverrides`), as the backend has them.
 */
import type { StubReply } from "./stub-places.ts";

type Json = Record<string, unknown>;

const POSTHOG = process.env.STUB_POSTHOG !== "off";

interface Settings {
  weights: number[];
  enabled: boolean;
  holdout: number | null;
}

interface Override {
  weights: number[] | null;
  enabled: boolean | null;
  holdout: number | null;
  changed_by: string;
  changed_at: string;
}

interface StubExperiment {
  brand: string;
  key: string;
  variants: string[];
  declared: Settings & { summary: string | null; declared_at: string };
  override: Override | null;
  weights_changed_at: string | null;
  retired: boolean;
}

const daysAgo = (d: number) => new Date(Date.now() - d * 86_400_000).toISOString();

const experiments: StubExperiment[] = [
  { brand: "aquafix", key: "hero_cta", variants: ["a", "b", "c"], declared: { weights: [1, 1, 1], enabled: true, holdout: null, summary: "A call button above the fold brings more calls", declared_at: daysAgo(1) }, override: null, weights_changed_at: null, retired: false },
  { brand: "aquafix", key: "lead_layout", variants: ["control", "short"], declared: { weights: [1, 1], enabled: true, holdout: 0.1, summary: null, declared_at: daysAgo(1) }, override: null, weights_changed_at: null, retired: false },
  { brand: "aquafix", key: "trust_badges", variants: ["off", "on"], declared: { weights: [1, 1], enabled: true, holdout: null, summary: "Badges under the form", declared_at: daysAgo(40) }, override: null, weights_changed_at: null, retired: true },
  {
    brand: "vifnet", key: "quote_form", variants: ["control", "short_form"],
    declared: { weights: [1, 1], enabled: true, holdout: null, summary: "A shorter quote form", declared_at: daysAgo(2) },
    override: { weights: [3, 1], enabled: null, holdout: null, changed_by: "colleague@example.test (stub)", changed_at: daysAgo(5) },
    weights_changed_at: daysAgo(5), retired: false,
  },
];

function effective(e: StubExperiment): Settings {
  return { weights: e.override?.weights ?? e.declared.weights, enabled: e.override?.enabled ?? e.declared.enabled, holdout: e.override?.holdout ?? e.declared.holdout };
}

function dto(e: StubExperiment): Json {
  const { summary, declared_at, ...settings } = e.declared;
  const q = encodeURIComponent(JSON.stringify({ kind: "InsightVizNode", source: { kind: "FunnelsQuery", experiment: e.key, brand_id: e.brand, from: e.weights_changed_at ?? declared_at } }));
  return {
    brand: e.brand, key: e.key, variants: e.variants, declared: { ...settings, summary, declared_at }, override: e.override, effective: effective(e),
    weights_changed_at: e.weights_changed_at, retired: e.retired, posthog_url: POSTHOG ? `https://us.posthog.com/project/0/insights/new#q=${q}` : null,
  };
}

const isNum = (v: unknown): v is number => typeof v === "number" && Number.isFinite(v);

/** The body's problem, as the backend words a 400; null when it is fine. */
function problem(body: Json, variants: number): string | null {
  const { weights, holdout, enabled } = body;
  if (weights !== undefined && weights !== null && !(Array.isArray(weights) && weights.length === variants && weights.every((w) => isNum(w) && w >= 0) && weights.reduce((a: number, b: number) => a + b, 0) > 0)) return "weights: one per variant, each 0 or more, summing above 0";
  if (holdout !== undefined && holdout !== null && !(isNum(holdout) && holdout >= 0 && holdout < 1)) return "holdout: in [0, 1)";
  if (enabled !== undefined && enabled !== null && typeof enabled !== "boolean") return "enabled: a boolean";
  return null;
}

export function experimentsRoute(method: string, path: string, query: URLSearchParams, body: Json, role: string, me: string): (StubReply & { changed?: string }) | null {
  if (path === "/experiments" && method === "GET") {
    const brand = query.get("brand");
    return { status: 200, body: { experiments: experiments.filter((e) => !brand || e.brand === brand).map(dto) } };
  }
  const m = path.match(/^\/experiments\/([^/]+)\/([^/]+)$/);
  if (!m) return null;
  if (method !== "PUT") return { status: 405, body: { error: "method not allowed" } };
  if (role !== "admin") return { status: 403, body: { error: "your role may not do this" } };
  const e = experiments.find((x) => x.brand === decodeURIComponent(m[1] ?? "") && x.key === decodeURIComponent(m[2] ?? ""));
  if (!e || e.retired) return { status: 404, body: { error: "not found" } };
  const why = problem(body, e.variants.length);
  if (why) return { status: 400, body: { error: why } };
  const before = effective(e).weights.join();
  const prev = e.override ?? { weights: null, enabled: null, holdout: null };
  const pick = <K extends "weights" | "enabled" | "holdout">(k: K) => (k in body ? (body[k] as Override[K]) : prev[k]);
  const next = { weights: pick("weights"), enabled: pick("enabled"), holdout: pick("holdout") };
  const at = new Date().toISOString();
  e.override = next.weights === null && next.enabled === null && next.holdout === null ? null : { ...next, changed_by: me, changed_at: at };
  if (effective(e).weights.join() !== before) e.weights_changed_at = at;
  return { status: 200, body: dto(e), changed: e.brand };
}
