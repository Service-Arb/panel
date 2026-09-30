import type { Share } from "@/shared/lib/share";
import { type Infer, type Parser, arrayOf, bool, nullable, num, object, oneOf, str } from "@/shared/lib/parse";

/** The personal stages 5–10 of the spec, as `panel_core::funnel::Totals::steps` lists them. */
export const FUNNEL_STAGES = ["created", "contacted", "quoted", "won", "completed", "paid"] as const;
export type FunnelStage = (typeof FUNNEL_STAGES)[number];

export const shareParser: Parser<Share> = object({ n: num, of: num, percent: nullable(num), small_sample: bool });

const stepParser = object({
  stage: oneOf(FUNNEL_STAGES),
  reached: num,
  of_previous: nullable(shareParser),
  of_leads: shareParser,
});
export type FunnelStep = Infer<typeof stepParser>;

export const funnelParser = object({
  from: str,
  to: str,
  brand: nullable(str),
  min_sample: num,
  stages: arrayOf(stepParser),
  lost: shareParser,
  manual: shareParser,
});
export type Funnel = Infer<typeof funnelParser>;

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
