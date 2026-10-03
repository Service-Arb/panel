import type { ChangedEvent } from "@/shared/lib/live";

/** A `pricing` change of this brand — or of any brand, when the event names none. */
export function followsPricing(brand: string | null): (event: ChangedEvent) => boolean {
  return (e) => e.topic === "pricing" && (brand === null || e.brand_id === null || e.brand_id === brand);
}

const timeOf = (at: string | null) => (at === null ? Number.NEGATIVE_INFINITY : Date.parse(at));

/**
 * Whether `read` holds a write made after `base` was read: someone else's,
 * since one's own re-pins. Strictly newer, so an older answer still on screen
 * while a re-read lands is not mistaken for one (a removal is a write too: it
 * keeps an `updated_at`).
 */
export function pricingSavedSince(read: { updated_at: string | null }, base: { updated_at: string | null }): boolean {
  return timeOf(read.updated_at) > timeOf(base.updated_at);
}
