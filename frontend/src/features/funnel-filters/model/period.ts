import { daysAgo, utcDay } from "@/shared/lib/format";

export const PERIODS = [7, 30, 90] as const;
export type Period = (typeof PERIODS)[number];
export const DEFAULT_PERIOD: Period = 30;

export function periodFrom(raw: string | null): Period {
  return PERIODS.find((p) => String(p) === raw) ?? DEFAULT_PERIOD;
}

/** The last `days` UTC days, today included — the backend's own default is 30 of them. */
export function rangeOf(period: Period, now: Date): { from: string; to: string } {
  return { from: utcDay(daysAgo(now, period - 1)), to: utcDay(now) };
}
