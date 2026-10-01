import { cn } from "@evinvest/uikit";

import { placeInterval } from "../lib/interval-scale";
import type { Difference } from "../model/experiment";

const W = 160;
const H = 16;
const MID = H / 2;

/**
 * The interval of a difference on an axis symmetric about zero, the zero line
 * dashed. Muted while it is not an answer; the estimate's tick only once it is.
 * Its numbers are in the text beside it, so the drawing is `aria-hidden` there.
 */
export function IntervalChart({ difference, axis, settled, className }: { difference: Difference; axis: number; settled: boolean; className?: string }) {
  const p = placeInterval(difference, settled ? difference.estimate : null, axis, W);
  return (
    <svg viewBox={`0 0 ${W} ${H}`} preserveAspectRatio="none" aria-hidden className={cn("h-4", className)}>
      <line x1={0} y1={MID} x2={W} y2={MID} className="stroke-border" vectorEffect="non-scaling-stroke" />
      <line x1={p.zero} y1={1} x2={p.zero} y2={H - 1} className="stroke-ink-soft" strokeDasharray="2 2" vectorEffect="non-scaling-stroke" />
      <rect x={p.from} y={MID - 3} width={Math.max(1, p.to - p.from)} height={6} className={settled ? "fill-primary-ink/60" : "fill-ink-soft/40"} />
      {p.clippedLow && <polygon points={`0,${MID} 4,${MID - 4} 4,${MID + 4}`} className="fill-ink-soft" />}
      {p.clippedHigh && <polygon points={`${W},${MID} ${W - 4},${MID - 4} ${W - 4},${MID + 4}`} className="fill-ink-soft" />}
      {p.estimate !== null && <line x1={p.estimate} y1={2} x2={p.estimate} y2={H - 2} className="stroke-ink" strokeWidth={2} vectorEffect="non-scaling-stroke" />}
    </svg>
  );
}
