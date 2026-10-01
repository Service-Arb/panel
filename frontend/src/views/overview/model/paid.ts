import type { Paid } from "@/entities/funnel";
import { formatMoney } from "@/shared/lib/format";

export interface PaidLine {
  currency: string;
  billed: string;
  /** Null when nothing was kept: a zero would only lengthen the line. */
  commission: string | null;
}

/**
 * A line per currency, in whole units and never converted: a total across
 * currencies would need a rate the panel does not have (spec §10.1).
 */
export function paidLines(payments: readonly Paid[], locale: string): PaidLine[] {
  return payments.map((p) => ({
    currency: p.currency,
    billed: formatMoney(p.billed, p.currency, locale),
    commission: p.commission === 0 ? null : formatMoney(p.commission, p.currency, locale),
  }));
}
