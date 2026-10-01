"use client";

import { Badge } from "@evinvest/uikit";

import { type Comparison, IntervalChart, type Rate, formatPoints, readComparison } from "@/entities/experiment";
import { useLocale, useT } from "@/shared/i18n";
import { type Share, formatShare } from "@/shared/lib/share";

/**
 * A rate of one variant and, unless it is the control, the difference to the
 * control: the interval in points and drawn, the estimate only once the interval
 * clears zero, and "not enough data" with the reason while it does not.
 */
export function RateRow({ rate, share, comparison, axis }: { rate: Rate; share: Share; comparison: Comparison | null; axis: number }) {
  const t = useT();
  return (
    <div className="grid grid-cols-(--grid-exp-rate) items-center gap-x-3 gap-y-1 text-sm md:grid-cols-(--grid-exp-rate-md)">
      <dt className="min-w-0 text-ink-mid">{t(`experiments.rate.${rate}`)}</dt>
      <dd className="text-right tabular-nums text-ink">{formatShare(share, t)}</dd>
      <dd className="col-span-full min-w-0 md:col-span-1">{comparison && <ComparisonLine comparison={comparison} axis={axis} />}</dd>
    </div>
  );
}

function ComparisonLine({ comparison, axis }: { comparison: Comparison; axis: number }) {
  const t = useT();
  const locale = useLocale();
  const reading = readComparison(comparison);
  const d = reading.interval;
  const points = (v: number) => formatPoints(v, d?.decimals ?? 0, locale);
  return (
    <div className="flex flex-wrap items-center gap-x-3 gap-y-1">
      {d && <IntervalChart difference={d} axis={axis} settled={reading.showEstimate} className="w-40 max-w-full shrink-0" />}
      {d && (
        <span className="whitespace-nowrap tabular-nums text-ink-mid">
          {reading.showEstimate ? t("experiments.estimate", { estimate: points(d.estimate), low: points(d.low), high: points(d.high) }) : t("experiments.interval", { low: points(d.low), high: points(d.high) })}
        </span>
      )}
      {reading.badge && (
        <Badge variant="outline" className="border-accent-warn/40 text-accent-warn">
          {t(`experiments.insufficient.${reading.badge}`)}
        </Badge>
      )}
    </div>
  );
}
