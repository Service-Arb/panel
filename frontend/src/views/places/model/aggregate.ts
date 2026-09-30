import { type Lead, type LeadPage, reached } from "@/entities/lead";

/** RFC 3339 instants compared as instants: the backend's and the browser's spell fractions differently. */
function before(a: string, b: string): boolean {
  return Date.parse(a) < Date.parse(b);
}

export interface PlaceRow {
  brand: string;
  location: string | null;
  leads: number;
  contacted: number;
  won: number;
  paid: number;
}

/**
 * Per-location counts of the leads that came in since `fromIso`, by the stages
 * each reached — the same rule the backend's funnel sums by.
 */
export function aggregatePlaces(leads: readonly Lead[], fromIso: string): PlaceRow[] {
  const rows = new Map<string, PlaceRow>();
  for (const lead of leads) {
    if (!lead.created_at || before(lead.created_at, fromIso)) continue;
    const key = `${lead.brand}\u0000${lead.location ?? ""}`;
    const row = rows.get(key) ?? { brand: lead.brand, location: lead.location, leads: 0, contacted: 0, won: 0, paid: 0 };
    row.leads += 1;
    if (reached(lead, "contacted")) row.contacted += 1;
    if (reached(lead, "won")) row.won += 1;
    if (reached(lead, "paid")) row.paid += 1;
    rows.set(key, row);
  }
  return [...rows.values()].sort((a, b) => b.leads - a.leads || a.brand.localeCompare(b.brand));
}

/** How many leads the browser is willing to page through before giving up on counting. */
export const PLACES_LIMIT = 1000;
const PAGE = 200;

/**
 * Pages newest-first until a lead older than `fromIso` shows up (the rest are
 * older still) or the list ends. Past `limit` leads it stops and says the
 * counts would be incomplete — then a backend endpoint is needed, not more pages.
 */
export async function loadWindow(fetchPage: (cursor: string | null, limit: number) => Promise<LeadPage>, fromIso: string, limit = PLACES_LIMIT): Promise<{ leads: Lead[]; complete: boolean }> {
  const leads: Lead[] = [];
  let cursor: string | null = null;
  do {
    const page: LeadPage = await fetchPage(cursor, PAGE);
    leads.push(...page.leads);
    const oldest = page.leads.at(-1)?.created_at;
    if (!page.next_cursor || (oldest && before(oldest, fromIso))) return { leads, complete: true };
    cursor = page.next_cursor;
  } while (leads.length < limit);
  return { leads, complete: false };
}
