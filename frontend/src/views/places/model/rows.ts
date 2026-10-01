import type { FunnelStage, LocationSlice } from "@/entities/funnel";
import type { Share } from "@/shared/lib/share";

/** The steps a location's card shows, each as a share of its leads. */
export const PLACE_STEPS = ["contacted", "won", "paid"] as const satisfies readonly FunnelStage[];
export type PlaceStep = (typeof PLACE_STEPS)[number];

export interface PlaceRow {
  brand: string;
  location: string | null;
  leads: number;
  steps: { stage: PlaceStep; share: Share }[];
}

/**
 * A card per location of `/funnel?by=location`, busiest first. The shares are
 * the backend's (`of_leads`), percent withheld below its minimum sample — none
 * is counted here.
 */
export function placeRows(slices: readonly LocationSlice[]): PlaceRow[] {
  const rows = slices.map((s) => ({
    brand: s.brand,
    location: s.location,
    leads: s.stages.find((step) => step.stage === "created")?.reached ?? 0,
    steps: PLACE_STEPS.flatMap((stage) => {
      const step = s.stages.find((x) => x.stage === stage);
      return step ? [{ stage, share: step.of_leads }] : [];
    }),
  }));
  return rows.sort((a, b) => b.leads - a.leads || a.brand.localeCompare(b.brand) || (a.location ?? "").localeCompare(b.location ?? ""));
}
