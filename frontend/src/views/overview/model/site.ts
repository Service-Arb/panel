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

export interface IntentToLead {
  leads: number;
  intents: number;
}

/**
 * Intents (PostHog, per day) beside leads (the panel, per lead) — an estimate
 * across the edge, and only ever the two counts: an intent is a click, not a
 * person, and a lead may have come by phone with no click at all, so no percent
 * is drawn across it at any size (§10.1). Nothing when no intent was counted.
 */
export function intentToLead(leads: number, intents: number): IntentToLead | null {
  return intents === 0 ? null : { leads, intents };
}
