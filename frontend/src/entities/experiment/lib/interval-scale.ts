import type { Difference } from "../model/experiment";

/**
 * The half-widths an axis may take, in points. Past the last one an interval is
 * cut at the edge rather than squeezing every other interval to a sliver: a wide
 * interval says "unknown" either way, and its numbers stay in the text beside it.
 */
export const AXIS_CAP = 20;
const STEPS = [1, 2, 5, 10, AXIS_CAP] as const;

/** One axis for an experiment, symmetric about zero, so its variants line up. */
export function axisFor(intervals: readonly Pick<Difference, "low" | "high">[]): number {
  const reach = Math.max(0, ...intervals.flatMap((d) => [Math.abs(d.low), Math.abs(d.high)]));
  return STEPS.find((s) => s >= reach) ?? AXIS_CAP;
}

export interface PlacedInterval {
  zero: number;
  from: number;
  to: number;
  estimate: number | null;
  /** The interval runs past the axis on that side and is drawn cut. */
  clippedLow: boolean;
  clippedHigh: boolean;
}

/** An interval in the chart's own units: `[-axis, axis]` onto `[0, width]`. */
export function placeInterval(d: Pick<Difference, "low" | "high">, estimate: number | null, axis: number, width: number): PlacedInterval {
  const x = (v: number) => ((Math.min(axis, Math.max(-axis, v)) + axis) / (2 * axis)) * width;
  return {
    zero: x(0),
    from: x(d.low),
    to: x(d.high),
    estimate: estimate === null ? null : x(estimate),
    clippedLow: d.low < -axis,
    clippedHigh: d.high > axis,
  };
}
