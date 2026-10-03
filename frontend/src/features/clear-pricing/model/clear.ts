import { type PricingItem, clearPricing, conflictCurrent } from "@/entities/pricing";
import { ApiError } from "@/shared/api";

export type ClearOutcome =
  | { kind: "cleared"; item: PricingItem }
  /** Someone wrote first; `current` is what they wrote, when the 409 carried it. */
  | { kind: "conflict"; current: PricingItem | null }
  | { kind: "failed"; error: unknown };

/**
 * One DELETE, its answer kept: the screen pins what the server now holds
 * instead of reading it again, so its own removal is never taken for someone else's.
 */
export async function tryClear(item: PricingItem): Promise<ClearOutcome> {
  try {
    return { kind: "cleared", item: await clearPricing(item.brand_id, item.updated_at) };
  } catch (e) {
    if (e instanceof ApiError && e.failure.kind === "conflict") return { kind: "conflict", current: conflictCurrent(e.failure.body) };
    return { kind: "failed", error: e };
  }
}
