import type { Comparison, Difference } from "../model/experiment";

export type InsufficientReason = NonNullable<Comparison["reason"]>;

/** What one comparison may show: no more than the backend's judgement allows. */
export interface ComparisonReading {
  /** The interval, when there is one: never while an arm is too small. */
  interval: Difference | null;
  /** The point estimate only once the interval clears zero: a "+3" beside a −2…+8 reads as a result. */
  showEstimate: boolean;
  /** Why it is not an answer yet; null when it is one. */
  badge: InsufficientReason | null;
}

export function readComparison(c: Comparison): ComparisonReading {
  const badge = c.insufficient ? (c.reason ?? (c.difference ? "interval_includes_zero" : "small_sample")) : null;
  const interval = badge === "small_sample" ? null : c.difference;
  return { interval, showEstimate: interval !== null && badge === null, badge };
}

/** Percentage points to the backend's rounding, signed, with a true minus: "−2", "+0.4", "0". */
export function formatPoints(value: number, decimals: number, locale: string): string {
  const text = new Intl.NumberFormat(locale, { minimumFractionDigits: decimals, maximumFractionDigits: decimals, signDisplay: "exceptZero" }).format(value);
  return text.replace("-", "−");
}
