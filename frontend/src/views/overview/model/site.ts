/** How many named sources a breakdown shows before the rest go into "other". */
export const TOP_SOURCES = 3;

export interface SourceBreakdown {
  top: { source: string; n: number }[];
  /** Visits from every source past the top ones; 0 when there are none. */
  others: number;
}

/**
 * The biggest sources of visits, then the rest in one sum. None when fewer than
 * two sources brought anything: "all from google" changes no decision, and a
 * line that says it is a line nobody reads (spec §10.1).
 */
export function sourceBreakdown(bySource: Readonly<Record<string, number>>, top = TOP_SOURCES): SourceBreakdown | null {
  const ranked = Object.entries(bySource)
    .filter(([, n]) => n > 0)
    .map(([source, n]) => ({ source, n }))
    .sort((a, b) => b.n - a.n || a.source.localeCompare(b.source));
  if (ranked.length < 2) return null;
  return { top: ranked.slice(0, top), others: ranked.slice(top).reduce((sum, s) => sum + s.n, 0) };
}

export type IntentToLead = { kind: "percent"; percent: number; leads: number; intents: number } | { kind: "counts"; leads: number; intents: number };

/**
 * Intents (PostHog, per day) against leads (the panel, per lead) — an estimate
 * across the edge, never an exact rate: an intent is a click, not a person, and
 * a lead may have come by phone with no click at all. A whole percent only from
 * `minSample` intents on, and only while it can be a share at all; otherwise the
 * two counts side by side. Nothing when no intent was counted.
 */
export function intentToLead(leads: number, intents: number, minSample: number): IntentToLead | null {
  if (intents === 0) return null;
  if (intents < minSample || leads > intents) return { kind: "counts", leads, intents };
  return { kind: "percent", percent: Math.round((leads * 100) / intents), leads, intents };
}
