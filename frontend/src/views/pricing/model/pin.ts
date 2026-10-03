import type { PricingItem } from "@/entities/pricing";

/**
 * What to pin as the editor's base, if anything: only once nothing is pinned
 * and the read is the one asked for. Right after a write that came back
 * without the pricing, the read still shows the answer from before it; pinning
 * that would make the write itself look like someone else's.
 */
export function pinFrom(pinned: PricingItem | null, read: PricingItem | null, fresh: boolean): PricingItem | null {
  return pinned === null && fresh ? read : null;
}
