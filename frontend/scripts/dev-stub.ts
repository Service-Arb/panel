/**
 * A stand-in for the panel's backend, for `next dev` without Rust, Postgres or
 * concierge: the `/api/v1` operator API and `/auth/*` over an in-memory set of
 * leads. Same shapes, same gate (401 without a session, CSRF on writes).
 *
 *   npm run dev:stub                  # :3121, signed in as an operator
 *   STUB_ROLE=admin npm run dev:stub  # as an admin
 *   STUB_ME=403 npm run dev:stub      # the gate refusing: 401 | 403 | 503
 *   STUB_MIN_SAMPLE=2 npm run dev:stub  # percents (and bars) from 2 leads, not 30
 *
 * then `npm run dev` in another shell. Data is made up and says so ("stub").
 */
import { randomUUID } from "node:crypto";
import { type IncomingMessage, type ServerResponse, createServer } from "node:http";

const PORT = Number(process.env.STUB_PORT ?? 3121);
const ROLE = process.env.STUB_ROLE === "admin" ? "admin" : "operator";
const ME_FAILURE = process.env.STUB_ME;
const CSRF = "stub-csrf";
const MIN_SAMPLE = Number(process.env.STUB_MIN_SAMPLE ?? 30);

type Json = Record<string, unknown>;
const now = () => new Date();
const iso = (d: Date) => d.toISOString();
const minsAgo = (m: number) => iso(new Date(now().getTime() - m * 60_000));

interface StubLead {
  brand: string;
  lead_id: string;
  location: string | null;
  stage: string;
  manual: boolean;
  times: Record<string, string>;
  lost_reason: string | null;
  pii: Json;
  events: Json[];
  payments: { billed: number; commission: number; currency: string }[];
}

function makeLead(i: number, stage: string, minutes: number, pii: Json, location: string | null = "lyon-3", brand = "aquafix"): StubLead {
  const created = minsAgo(minutes);
  const order = ["created", "contacted", "quoted", "won", "completed", "paid"];
  const times: Record<string, string> = { created_at: created };
  for (const s of order.slice(1, order.indexOf(stage) + 1)) times[`${s}_at`] = minsAgo(minutes - 10);
  return { brand, lead_id: `stub-${i}`, location, stage, manual: false, times, lost_reason: null, pii, events: [event("lead.created", { channel: "form" }, "site", created)], payments: [] };
}

function event(type: string, properties: Json, kind = "panel", at = iso(now())): Json {
  return { id: randomUUID(), type, type_version: 1, occurred_at: at, received_at: at, source_kind: kind, source_id: kind === "panel" ? "panel" : "aquafix-site", manual: kind === "panel", job_id: null, status: "accepted", status_reason: null, properties, pii: null };
}

const leads: StubLead[] = [
  makeLead(1, "created", 42, { name: "Camille (stub)", phone: "+33 6 00 00 00 01", need: "Leak under the sink" }, "lyon-7"),
  makeLead(2, "created", 6, { name: "Hugo (stub)", phone: "+33 6 00 00 00 02", need: "Blocked bathroom drain" }),
  makeLead(3, "contacted", 60 * 26, { name: "Léa (stub)", phone: "+33 6 00 00 00 03", need: "Boiler not heating" }),
  makeLead(4, "quoted", 60 * 50, { name: "Noah (stub)", need: "Replace the toilet" }, "lyon-7"),
  makeLead(5, "won", 60 * 80, { phone: "+33 6 00 00 00 05", need: "Drain cleaning" }),
  makeLead(6, "paid", 60 * 200, { name: "Emma (stub)", need: "Shower repair" }, "lyon-7"),
  makeLead(7, "created", 3, { need: "Office cleaning 120 m²" }, "paris-11", "vifnet"),
  makeLead(8, "paid", 60 * 300, { name: "Jules (stub)", need: "New water heater" }),
  makeLead(9, "contacted", 60 * 30, { need: "Called without saying where" }, null),
];
leads[5]!.payments.push({ billed: 23_100, commission: 2_310, currency: "EUR" });
leads[7]!.payments.push({ billed: 208_000, commission: 20_800, currency: "EUR" }, { billed: 9_050, commission: 0, currency: "GBP" });

const sources: Json[] = [{ key_id: "aquafix-site", kind: "site", brands: ["aquafix"], created_at: minsAgo(60 * 24 * 10), revoked_at: null }];

function leadDto(l: StubLead): Json {
  const t = (k: string) => l.times[k] ?? null;
  const waiting = l.stage === "created" && !t("contacted_at");
  const since = t("created_at") ?? iso(now());
  const secs = Math.floor((now().getTime() - Date.parse(since)) / 1000);
  return {
    brand: l.brand, lead_id: l.lead_id, location: l.location, job_id: null, stage: l.stage, channel: "form", manual: l.manual,
    created_at: t("created_at"), contacted_at: t("contacted_at"), quoted_at: t("quoted_at"), won_at: t("won_at"),
    completed_at: t("completed_at"), paid_at: t("paid_at"), lost_at: t("lost_at"), lost_reason: l.lost_reason,
    last_event_at: String(l.events.at(-1)?.occurred_at ?? since),
    sla: waiting ? { waiting_since: since, waiting_seconds: secs, overdue: secs > 30 * 60 } : null,
    pii: l.pii,
  };
}

function share(n: number, of: number): Json {
  const percent = of >= MIN_SAMPLE && of > 0 ? Math.floor((n * 100 + Math.floor(of / 2)) / of) : null;
  return { n, of, percent, small_sample: percent === null };
}

/** A slice's numbers: steps, lost and manual shares, and payments summed per currency, never converted. */
function slice(ls: StubLead[]): Json {
  const has = (k: string) => ls.filter((l) => l.times[`${k}_at`]).length;
  const counts: [string, number][] = [["created", ls.length], ["contacted", has("contacted")], ["quoted", has("quoted")], ["won", has("won")], ["completed", has("completed")], ["paid", has("paid")]];
  const paid = new Map<string, { currency: string; billed: number; commission: number; count: number }>();
  for (const p of ls.flatMap((l) => l.payments)) {
    const row = paid.get(p.currency) ?? { currency: p.currency, billed: 0, commission: 0, count: 0 };
    row.billed += p.billed;
    row.commission += p.commission;
    row.count += 1;
    paid.set(p.currency, row);
  }
  return {
    stages: counts.map(([stage, n], i) => ({ stage, reached: n, of_previous: i ? share(n, counts[i - 1]![1]) : null, of_leads: share(n, ls.length) })),
    lost: share(ls.filter((l) => l.stage === "lost").length, ls.length), manual: share(ls.filter((l) => l.manual).length, ls.length),
    payments: [...paid.values()].sort((a, b) => a.currency.localeCompare(b.currency)),
  };
}

function funnel(brand: string | null, by: string | null): Json {
  const ls = leads.filter((l) => !brand || l.brand === brand);
  const head = { from: iso(new Date(now().getTime() - 29 * 86_400_000)).slice(0, 10), to: iso(now()).slice(0, 10), brand, min_sample: MIN_SAMPLE };
  if (by !== "location") return { ...head, ...slice(ls) };
  const places = [...new Set(ls.map((l) => `${l.brand}/${l.location ?? ""}`))].sort();
  const locations = places.map((key) => {
    const [b, loc] = key.split("/") as [string, string];
    return { brand: b, location: loc || null, ...slice(ls.filter((l) => l.brand === b && (l.location ?? "") === loc)) };
  });
  return { ...head, by: "location", locations };
}

/** Every location a lead names, with the time of its latest lead. */
function places(): Json {
  const latest = new Map<string, { brand: string; location: string; last_lead_at: string | null }>();
  for (const l of leads) {
    if (!l.location) continue;
    const key = `${l.brand}/${l.location}`;
    const at = l.times.created_at ?? null;
    const prev = latest.get(key);
    if (!prev || (at && (!prev.last_lead_at || at > prev.last_lead_at))) latest.set(key, { brand: l.brand, location: l.location, last_lead_at: at });
  }
  return { places: [...latest.values()] };
}

/** Leads by current stage under a place filter, every stage present. */
function counts(brand: string | null, location: string | null): Json {
  const ls = leads.filter((l) => (!brand || l.brand === brand) && (!location || l.location === location));
  const stages = Object.fromEntries(["created", "contacted", "quoted", "won", "completed", "paid", "lost"].map((s) => [s, ls.filter((l) => l.stage === s).length]));
  const overdue = ls.map(leadDto).filter((l) => (l.sla as Json | null)?.overdue === true).length;
  return { stages, overdue, total: ls.length };
}

function send(res: ServerResponse, status: number, body?: unknown, headers: Record<string, string | string[]> = {}): void {
  res.writeHead(status, { "content-type": "application/json", "cache-control": "no-store", ...headers });
  res.end(body === undefined ? undefined : JSON.stringify(body));
}

async function readJson(req: IncomingMessage): Promise<Json> {
  const chunks: Buffer[] = [];
  for await (const c of req) chunks.push(c as Buffer);
  const text = Buffer.concat(chunks).toString();
  return text ? (JSON.parse(text) as Json) : {};
}

function signedIn(req: IncomingMessage): boolean {
  return /(?:^|;\s*)sa_session=/.test(req.headers.cookie ?? "");
}

/** `created_from`/`created_to` are UTC days, both included. */
function inDays(at: string, from: string | null, to: string | null): boolean {
  const day = at.slice(0, 10);
  return (!from || day >= from) && (!to || day <= to);
}

function find(brand: string, id: string): StubLead | undefined {
  return leads.find((l) => l.brand === brand && l.lead_id === id);
}

async function api(req: IncomingMessage, res: ServerResponse, url: URL): Promise<void> {
  const path = url.pathname.replace(/^\/api\/v1/, "");
  const write = req.method !== "GET";
  if (write && req.headers["x-sa-csrf"] !== CSRF) return send(res, 403, { error: "csrf" });
  if (ME_FAILURE === "403") return send(res, 403, { error: "no access to the panel" });
  if (ME_FAILURE === "503") return send(res, 503, { error: "sign-in is unavailable, try again" });
  if (ME_FAILURE === "401" || !signedIn(req)) return send(res, 401, { error: "sign in" }, { "set-cookie": "sa_session=; Path=/; Max-Age=0" });

  if (path === "/me") return send(res, 200, { user_id: "00000000-0000-7000-8000-000000000001", role: ROLE, email: "stub@example.test", preferred_name: `Stub ${ROLE}` });
  if (path === "/funnel") return send(res, 200, funnel(url.searchParams.get("brand"), url.searchParams.get("by")));
  if (path === "/places") return send(res, 200, places());
  if (path === "/leads/counts") return send(res, 200, counts(url.searchParams.get("brand"), url.searchParams.get("location")));
  if (path === "/leads" && req.method === "GET") {
    const q = url.searchParams;
    const list = leads
      .filter((l) => (!q.get("stage") || l.stage === q.get("stage")) && (!q.get("brand") || l.brand === q.get("brand")) && (!q.get("location") || l.location === q.get("location")))
      .map(leadDto)
      .filter((l) => q.get("overdue") !== "true" || (l.sla as Json | null)?.overdue === true)
      .filter((l) => inDays(String(l.created_at), q.get("created_from"), q.get("created_to")))
      .sort((a, b) => String(b.created_at).localeCompare(String(a.created_at)));
    return send(res, 200, { leads: list, next_cursor: null });
  }
  if (path === "/leads" && req.method === "POST") {
    const b = await readJson(req);
    const l = makeLead(leads.length + 1, "created", 0, { need: b.need, ...(b.phone ? { phone: b.phone } : {}) }, String(b.location), String(b.brand));
    l.lead_id = `p-${randomUUID()}`;
    l.manual = true;
    leads.push(l);
    return send(res, 201, { brand: l.brand, lead_id: l.lead_id, event_id: randomUUID() });
  }
  if (path === "/sources" && ROLE !== "admin") return send(res, 403, { error: "your role may not do this" });
  if (path === "/sources" && req.method === "GET") return send(res, 200, { sources });
  if (path === "/sources" && req.method === "POST") {
    const b = await readJson(req);
    if (sources.some((s) => s.key_id === b.key_id)) return send(res, 409, { error: `a source ${String(b.key_id)} exists already` });
    sources.push({ key_id: b.key_id, kind: b.kind, brands: b.brands, created_at: iso(now()), revoked_at: null });
    return send(res, 201, { key_id: b.key_id, secret: `stub-secret-${randomUUID()}` });
  }
  const revoke = path.match(/^\/sources\/([^/]+)$/);
  if (revoke && req.method === "DELETE") {
    const s = sources.find((x) => x.key_id === decodeURIComponent(revoke[1]!));
    if (!s) return send(res, 404, { error: "not found" });
    s.revoked_at = iso(now());
    return send(res, 204);
  }

  const m = path.match(/^\/leads\/([^/]+)\/([^/]+)(\/.*)?$/);
  const lead = m ? find(decodeURIComponent(m[1]!), decodeURIComponent(m[2]!)) : undefined;
  if (!m || !lead) return send(res, 404, { error: "not found" });
  const rest = m[3] ?? "";
  if (rest === "" && req.method === "GET") return send(res, 200, { lead: leadDto(lead), events: lead.events });
  if (rest === "/stage") {
    const b = await readJson(req);
    const stage = String(b.stage);
    lead.stage = stage;
    lead.times[`${stage}_at`] = iso(now());
    if (stage === "lost") lead.lost_reason = String(b.reason);
    lead.events.push(event(stage === "won" || stage === "completed" ? `job.${stage}` : `lead.${stage}`, b));
    return send(res, 201, { event_id: randomUUID() });
  }
  if (rest === "/calls/attempt") {
    const e = event("call.attempted", {});
    lead.events.push(e);
    return send(res, 201, { attempt_id: e.id });
  }
  if (/^\/calls\/[^/]+\/outcome$/.test(rest)) {
    const b = await readJson(req);
    lead.events.push(event("call.logged", { outcome: b.outcome }));
    return send(res, 201, { event_id: randomUUID() });
  }
  if (rest === "/payments") {
    const b = await readJson(req);
    lead.stage = "paid";
    lead.times.paid_at = iso(now());
    lead.payments.push({ billed: Number(b.billed), commission: Number(b.commission), currency: String(b.currency) });
    lead.events.push(event("payment.received", b));
    return send(res, 201, { event_id: randomUUID() });
  }
  return send(res, 404, { error: "not found" });
}

createServer((req, res) => {
  const url = new URL(req.url ?? "/", `http://${req.headers.host ?? "localhost"}`);
  if (url.pathname === "/auth/login") {
    // No concierge: signing in is instant. Plain names, as the backend uses over http.
    res.writeHead(303, { location: "/", "set-cookie": ["sa_session=stub; Path=/; HttpOnly; SameSite=Lax", `sa_csrf=${CSRF}; Path=/; SameSite=Lax`] });
    return res.end();
  }
  if (url.pathname === "/auth/logout" && req.method === "POST") {
    return send(res, 204, undefined, { "set-cookie": ["sa_session=; Path=/; Max-Age=0", "sa_csrf=; Path=/; Max-Age=0"] });
  }
  if (url.pathname.startsWith("/api/v1/")) {
    api(req, res, url).catch((e: unknown) => send(res, 500, { error: String(e) }));
    return;
  }
  send(res, 404, { error: "not found" });
}).listen(PORT, "127.0.0.1", () => console.log(`panel dev stub on http://127.0.0.1:${PORT} as ${ROLE}`));
