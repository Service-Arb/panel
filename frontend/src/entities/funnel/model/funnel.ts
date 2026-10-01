import { type Infer, arrayOf, nullable, num, object, oneOf, str } from "@/shared/lib/parse";
import { aggregateSourceParser } from "@/shared/lib/aggregate-source";
import { shareParser } from "@/shared/lib/share";

import { aggregateParser } from "./aggregate";

/** The personal stages 5–10 of the spec, as `panel_core::funnel::Totals::steps` lists them. */
export const FUNNEL_STAGES = ["created", "contacted", "quoted", "won", "completed", "paid"] as const;
export type FunnelStage = (typeof FUNNEL_STAGES)[number];

const stepParser = object({
  stage: oneOf(FUNNEL_STAGES),
  reached: num,
  of_previous: nullable(shareParser),
  of_leads: shareParser,
});
export type FunnelStep = Infer<typeof stepParser>;

/**
 * What a slice's leads were paid in one currency, in its minor units. One row per
 * currency and never converted: a sum across currencies would be a number of ours.
 */
const paidParser = object({ currency: str, billed: num, commission: num, count: num });
export type Paid = Infer<typeof paidParser>;

/**
 * A slice's own numbers — the whole funnel's, or one location's: the per-lead
 * stages 5–10 and, beside them and never divided by them, the per-day 3–4.
 */
const slice = {
  stages: arrayOf(stepParser),
  aggregate: aggregateParser,
  lost: shareParser,
  manual: shareParser,
  payments: arrayOf(paidParser),
};

export const funnelParser = object({ from: str, to: str, brand: nullable(str), min_sample: num, aggregate_source: aggregateSourceParser, ...slice });
export type Funnel = Infer<typeof funnelParser>;

/** One location's slice; `location` is null for the leads that name none. */
const locationSliceParser = object({ brand: str, location: nullable(str), ...slice });
export type LocationSlice = Infer<typeof locationSliceParser>;

/** `GET /funnel?by=location`: a row per location with leads or visits in the window, none for an empty one. */
export const funnelByLocationParser = object({
  from: str,
  to: str,
  brand: nullable(str),
  min_sample: num,
  aggregate_source: aggregateSourceParser,
  by: oneOf(["location"]),
  locations: arrayOf(locationSliceParser),
});
export type FunnelByLocation = Infer<typeof funnelByLocationParser>;

/**
 * The step that loses the most leads, counted in leads (a percent may be
 * withheld for a small sample, a count never is). None when nothing is lost,
 * and none on a tie — two "biggest" losses would point nowhere.
 */
export function biggestLoss(steps: readonly FunnelStep[]): FunnelStage | null {
  let best: { stage: FunnelStage; lost: number } | null = null;
  let tie = false;
  for (const step of steps) {
    if (!step.of_previous) continue;
    const lost = step.of_previous.of - step.reached;
    if (lost <= 0) continue;
    if (!best || lost > best.lost) {
      best = { stage: step.stage, lost };
      tie = false;
    } else if (lost === best.lost) tie = true;
  }
  return best && !tie ? best.stage : null;
}
