import type { Lead } from "@/entities/lead";

/** A row's identity across reads: brand and lead. */
export const leadKey = (lead: Pick<Lead, "brand" | "lead_id">) => `${lead.brand}/${lead.lead_id}`;

/** What a person would notice changed on a row: the stage, a new event (a call, a payment), a loss reason, the booking. */
export function differs(a: Lead, b: Lead): boolean {
  const moved = a.stage !== b.stage || a.last_event_at !== b.last_event_at || a.lost_reason !== b.lost_reason || a.paid_at !== b.paid_at;
  // A provider's booking changes the row without a lead event of its own.
  return moved || a.booking.status !== b.booking.status || a.booking.start_at !== b.booking.start_at;
}

/** Rows on screen with each update put in place — never moved, never added. */
export function applyUpdates(shown: readonly Lead[], updates: readonly Lead[]): { leads: Lead[]; changed: string[] } {
  const byKey = new Map(updates.map((l) => [leadKey(l), l]));
  const changed: string[] = [];
  const leads = shown.map((row) => {
    const next = byKey.get(leadKey(row));
    if (!next) return row;
    if (differs(row, next)) changed.push(leadKey(row));
    return next;
  });
  return { leads, changed };
}

/** Leads in a fresh first page that the screen does not show: arrivals, or leads that entered the filter. */
export function arrivals(shown: readonly Lead[], fresh: readonly Lead[]): Lead[] {
  const on = new Set(shown.map(leadKey));
  return fresh.filter((l) => !on.has(leadKey(l)));
}

const createdOf = (l: Lead) => l.created_at ?? "";

/**
 * Rows on screen that a fresh first page should hold and does not: they left
 * the filter (a new lead contacted, under "new"). Only within the page's span —
 * a complete page spans everything, a full one only down to its oldest row.
 */
export function leftFilter(shown: readonly Lead[], fresh: readonly Lead[], complete: boolean): Lead[] {
  const inFresh = new Set(fresh.map(leadKey));
  const oldest = fresh.reduce<string | null>((min, l) => (min === null || createdOf(l) < min ? createdOf(l) : min), null);
  return shown.filter((l) => !inFresh.has(leadKey(l)) && (complete || (oldest !== null && createdOf(l) >= oldest)));
}

/** The waiting arrivals with newer reads of them folded in, newest first, nothing twice. */
export function addWaiting(waiting: readonly Lead[], incoming: readonly Lead[]): Lead[] {
  const byKey = new Map<string, Lead>();
  for (const l of [...waiting, ...incoming]) byKey.set(leadKey(l), l);
  return newestFirst([...byKey.values()]);
}

/** What the banner offers: waiting arrivals a reload has not already put on screen. */
export function pendingOf(waiting: readonly Lead[], shown: readonly Lead[]): Lead[] {
  return arrivals(shown, waiting);
}

/** The person asked to see them: in their place by time, the list's order (newest first). */
export function reveal(shown: readonly Lead[], waiting: readonly Lead[]): Lead[] {
  return newestFirst([...pendingOf(waiting, shown), ...shown]);
}

function newestFirst(leads: Lead[]): Lead[] {
  // Stable: rows with the same time keep the order the server gave them.
  return leads.sort((a, b) => createdOf(b).localeCompare(createdOf(a)));
}
