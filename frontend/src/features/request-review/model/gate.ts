import type { Lead } from "@/entities/lead";

export type Gate =
  | { kind: "hidden" }
  /** Asked already: the line says when and where. */
  | { kind: "asked"; at: string; channel: Lead["review_requested_channel"] }
  /** Could be asked, but the person may not see the contact it needs. */
  | { kind: "needs_pii" }
  | { kind: "ready" };

/** The job was done, whatever became of the lead after: a lost lead that was completed still counts. */
export function finishedJob(lead: Pick<Lead, "completed_at" | "paid_at">): boolean {
  return lead.completed_at !== null || lead.paid_at !== null;
}

/**
 * What the card shows for asking a review. The button needs a finished job and
 * a review link at the place; without the right to see contacts it is shown
 * disabled, with the reason, rather than missing.
 */
export function reviewGate(lead: Lead, reviewUrl: string | null, canSeePii: boolean): Gate {
  if (lead.review_requested_at !== null) return { kind: "asked", at: lead.review_requested_at, channel: lead.review_requested_channel };
  if (!finishedJob(lead) || !reviewUrl) return { kind: "hidden" };
  return canSeePii ? { kind: "ready" } : { kind: "needs_pii" };
}
