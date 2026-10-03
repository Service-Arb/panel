/**
 * Brand pricing for the dev stub: `/api/v1/pricing` over two brands — vifnet
 * with kitstart's `valid/cleaning.json` model, aquafix with none (its sites on
 * their baked prices). In memory, with a journal for the history.
 *
 *   STUB_PRICING_CONFLICT=1        every save and removal answers 409, as if someone saved first
 *   STUB_PRICING_INVALID=<path>    every save and preview answers 422 at that path
 *                                  (e.g. `inputs.bedrooms.options.studio.labels.en`)
 *
 * The validation is rough — enough to answer 422 with a path the editor can
 * show — and the preview's price is a STUB: base, adds, then multipliers and
 * discounts rounded per step, then `roundToCents` and `minimumCents`. It is not
 * kitstart's `priceOf`; the backend's is, and that is the one held to the fixtures.
 */
import { randomUUID } from "node:crypto";
import { readFileSync } from "node:fs";

import type { StubReply } from "./stub-places.ts";

type Json = Record<string, unknown>;

interface StubPricing {
  locales: string[];
  model: Json | null;
  updated_at: string | null;
  updated_by: string | null;
}

interface Change {
  id: string;
  at: string;
  by: string;
  model: Json | null;
}

const ALWAYS_CONFLICT = process.env.STUB_PRICING_CONFLICT === "1";
const FORCED_INVALID = process.env.STUB_PRICING_INVALID ?? null;
const SLUG = /^[a-z0-9_-]{1,40}$/;
const EFFECT: Record<string, string> = { add: "addCents", multiply: "multiplyBp", discount: "discountBp" };

const cleaning = JSON.parse(readFileSync(new URL("../tests/fixtures/pricing/valid/cleaning.json", import.meta.url), "utf8")) as Json;
const iso = () => new Date().toISOString();
const daysAgo = (n: number) => new Date(Date.now() - n * 86_400_000).toISOString();

const brands = new Map<string, StubPricing>([
  ["aquafix", { locales: ["fr", "en"], model: null, updated_at: null, updated_by: null }],
  ["vifnet", { locales: ["fr", "en"], model: cleaning, updated_at: daysAgo(2), updated_by: "admin@example.test (stub)" }],
]);

const journal = new Map<string, Change[]>([["vifnet", [{ id: randomUUID(), at: daysAgo(2), by: "admin@example.test (stub)", model: cleaning }]]]);

const item = (brand: string, p: StubPricing): Json => ({ brand_id: brand, locales: p.locales, model: p.model, updated_at: p.updated_at, updated_by: p.updated_by });

const isObj = (v: unknown): v is Json => typeof v === "object" && v !== null && !Array.isArray(v);
const isInt = (v: unknown, min: number, max: number) => typeof v === "number" && Number.isSafeInteger(v) && v >= min && v <= max;

/**
 * The first thing wrong, with a path in the server's style — inputs and
 * options by slug, a need's inputs by index — or null. Rough on purpose:
 * the backend's validator is kitstart's rules; this only lets the editor's
 * 422 handling be clicked through.
 */
function firstProblem(m: unknown, locales: readonly string[]): { path: string; error: string } | null {
  if (FORCED_INVALID) return { path: FORCED_INVALID, error: "refused by STUB_PRICING_INVALID (stub)" };
  if (!isObj(m)) return { path: "model", error: "an object" };
  const known = ["format", "currency", "validFrom", "roundToCents", "minimumCents", "inputs", "needs"];
  const unknown = Object.keys(m).find((k) => !known.includes(k));
  if (unknown) return { path: unknown, error: "unknown field" };
  if (m.format !== 1) return { path: "format", error: "1" };
  if (m.currency !== "EUR") return { path: "currency", error: "EUR" };
  if (typeof m.validFrom !== "string" || !/^\d{4}-\d{2}-\d{2}$/.test(m.validFrom)) return { path: "validFrom", error: "a date, YYYY-MM-DD" };
  if (!isInt(m.roundToCents, 1, 100_000_000)) return { path: "roundToCents", error: "an integer from 1 to 100000000" };
  if (!isInt(m.minimumCents, 0, 100_000_000)) return { path: "minimumCents", error: "an integer from 0 to 100000000" };
  const inputs = Array.isArray(m.inputs) ? m.inputs.filter(isObj) : [];
  for (const input of inputs) {
    const at = `inputs.${String(input.id)}`;
    if (typeof input.id !== "string" || !SLUG.test(input.id)) return { path: `${at}.id`, error: "a slug, [a-z0-9_-]{1,40}" };
    const effect = EFFECT[String(input.kind)];
    if (!effect) return { path: `${at}.kind`, error: "add, multiply or discount" };
    for (const locale of locales) if (!isObj(input.labels) || typeof input.labels[locale] !== "string") return { path: `${at}.labels.${locale}`, error: `no "${locale}" label` };
    const options = Array.isArray(input.options) ? input.options.filter(isObj) : [];
    if (options.length === 0) return { path: `${at}.options`, error: "a list of 1 to 32" };
    for (const o of options) {
      const there = `${at}.options.${String(o.id)}`;
      if (typeof o.id !== "string" || !SLUG.test(o.id)) return { path: `${there}.id`, error: "a slug, [a-z0-9_-]{1,40}" };
      for (const locale of locales) if (!isObj(o.labels) || typeof o.labels[locale] !== "string") return { path: `${there}.labels.${locale}`, error: `no "${locale}" label` };
      if (!isInt(o[effect], 0, effect === "discountBp" ? 10_000 : effect === "multiplyBp" ? 100_000 : 100_000_000)) return { path: `${there}.${effect}`, error: "out of range" };
    }
  }
  if (!isObj(m.needs)) return { path: "needs", error: "an object by need slug" };
  for (const [slug, need] of Object.entries(m.needs)) {
    if (!SLUG.test(slug)) return { path: `needs.${slug}`, error: "the need must be a slug" };
    if (!isObj(need)) return { path: `needs.${slug}`, error: "an object" };
    if (need.kind === "fixed" && !isInt(need.cents, 0, 100_000_000)) return { path: `needs.${slug}.cents`, error: "an integer from 0 to 100000000" };
    if (need.kind === "estimate") {
      if (!isInt(need.baseCents, 0, 100_000_000)) return { path: `needs.${slug}.baseCents`, error: "an integer from 0 to 100000000" };
      const asked = Array.isArray(need.inputs) ? need.inputs : [];
      if (asked.length > 12) return { path: `needs.${slug}.inputs`, error: "a list of 0 to 12" };
      const missing = asked.findIndex((id) => !inputs.some((i) => i.id === id));
      if (missing >= 0) return { path: `needs.${slug}.inputs[${missing}]`, error: `no input "${String(asked[missing])}"` };
    }
  }
  return null;
}

/** STUB pricing: per-step `Math.round` on floats, not kitstart's integer half-up. Good enough to see a number move. */
function stubPrice(m: Json, need: string, answers: Json): number | null {
  const pricing = isObj(m.needs) ? m.needs[need] : undefined;
  if (!isObj(pricing)) return null;
  if (pricing.kind === "fixed") return Number(pricing.cents);
  const inputs = (Array.isArray(m.inputs) ? m.inputs.filter(isObj) : []) as Json[];
  const chosen: { kind: string; value: number }[] = [];
  for (const id of (pricing.inputs as unknown[]) ?? []) {
    const input = inputs.find((i) => i.id === id);
    const option = (input?.options as Json[] | undefined)?.find((o) => o.id === answers[String(id)]);
    if (!input || !option) return null;
    chosen.push({ kind: String(input.kind), value: Number(option[EFFECT[String(input.kind)] ?? ""]) });
  }
  let total = Number(pricing.baseCents);
  for (const c of chosen) if (c.kind === "add") total += c.value;
  for (const c of chosen) if (c.kind === "multiply") total = Math.round((total * c.value) / 10_000);
  for (const c of chosen) if (c.kind === "discount") total = Math.round((total * (10_000 - c.value)) / 10_000);
  const step = Number(m.roundToCents);
  total = Math.round(total / step) * step;
  return Math.max(total, Number(m.minimumCents));
}

function write(brand: string, p: StubPricing, by: string, model: Json | null): void {
  p.model = model;
  p.updated_at = iso();
  p.updated_by = by;
  journal.set(brand, [{ id: randomUUID(), at: p.updated_at, by, model }, ...(journal.get(brand) ?? [])]);
}

/** Someone else saves vifnet (the live stub's doing): its minimum flips between 49 € and 59 €. */
export function touchPricing(by: string): void {
  const p = brands.get("vifnet");
  if (!p?.model) return;
  write("vifnet", p, by, { ...p.model, minimumCents: p.model.minimumCents === 4900 ? 5900 : 4900 });
}

/**
 * The pricing routes, or null when `path` (after `/api/v1`) is not one of
 * them. The caller has passed the gate and the CSRF check already. `changed`
 * names the brand a write changed, for the live socket.
 */
export function pricingRoute(method: string, path: string, body: Json, role: string, me: string): (StubReply & { changed?: string }) | null {
  if (path === "/pricing" && method === "GET") return { status: 200, body: { items: [...brands].map(([b, p]) => item(b, p)) } };
  const m = path.match(/^\/pricing\/([^/]+)(\/changes|\/preview)?$/);
  if (!m) return null;
  const brand = decodeURIComponent(m[1] ?? "");
  const rest = m[2] ?? "";
  const p = brands.get(brand);
  if (!p) return { status: 404, body: { error: "not found" } };
  if (rest === "" && method === "GET") return { status: 200, body: item(brand, p) };
  if (rest === "/changes" && method === "GET") return { status: 200, body: { changes: journal.get(brand) ?? [] } };
  if (rest === "/preview" && method === "POST") {
    const problem = firstProblem(body.model, []);
    if (problem) return { status: 422, body: problem };
    return { status: 200, body: { cents: stubPrice(body.model as Json, String(body.need), isObj(body.inputs) ? body.inputs : {}), stub: "simplified price, not kitstart's priceOf" } };
  }
  if (rest !== "" || (method !== "PUT" && method !== "DELETE")) return { status: 405, body: { error: "method not allowed" } };
  if (role !== "admin") return { status: 403, body: { error: "your role may not do this" } };
  if (ALWAYS_CONFLICT || (body.expected_updated_at ?? null) !== p.updated_at) return { status: 409, body: { error: "stale", current: item(brand, p) } };
  if (method === "DELETE") {
    write(brand, p, me, null);
    return { status: 200, body: item(brand, p), changed: brand };
  }
  const problem = firstProblem(body.model, p.locales);
  if (problem) return { status: 422, body: problem };
  write(brand, p, me, body.model as Json);
  return { status: 200, body: item(brand, p), changed: brand };
}
