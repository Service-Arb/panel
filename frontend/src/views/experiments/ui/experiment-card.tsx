"use client";

import { Badge, Card, CardContent, CardDescription, CardHeader, CardTitle } from "@evinvest/uikit";

import { type Experiment, RATES, type Variant, axisFor, readComparison } from "@/entities/experiment";
import { useT } from "@/shared/i18n";

import { RateRow } from "./rate-row";

/** One experiment: its variants, the control first, each on one axis so their intervals line up. */
export function ExperimentCard({ experiment }: { experiment: Experiment }) {
  const t = useT();
  const intervals = experiment.variants.flatMap((v) => RATES.flatMap((r) => (v.vs_control ? [readComparison(v.vs_control[r]).interval] : []))).filter((d) => d !== null);
  const axis = axisFor(intervals);
  return (
    <Card className="gap-3 py-4">
      <CardHeader className="px-4">
        <CardTitle className="break-words text-base text-ink">
          {experiment.experiment} · {experiment.brand}
        </CardTitle>
        <CardDescription className="tabular-nums">{t("experiments.days", { from: experiment.first_day, to: experiment.last_day })}</CardDescription>
      </CardHeader>
      <CardContent className="px-4">
        <ol className="flex flex-col divide-y divide-border">
          {experiment.variants.map((v) => (
            <VariantItem key={v.variant} variant={v} axis={axis} />
          ))}
        </ol>
      </CardContent>
    </Card>
  );
}

function VariantItem({ variant, axis }: { variant: Variant; axis: number }) {
  const t = useT();
  return (
    <li className="flex flex-col gap-2 py-3">
      <div className="flex flex-wrap items-center gap-x-2 gap-y-1">
        <span className="break-all font-medium text-ink">{variant.variant}</span>
        {variant.control && <Badge variant="outline">{t("experiments.control")}</Badge>}
        <span className="ml-auto text-xs tabular-nums text-ink-soft">{t("experiments.counts", { exposures: variant.exposures, leads: variant.leads })}</span>
      </div>
      <dl className="flex flex-col gap-2">
        {RATES.map((rate) => (
          <RateRow key={rate} rate={rate} share={variant.rates[rate]} comparison={variant.vs_control?.[rate] ?? null} axis={axis} />
        ))}
      </dl>
    </li>
  );
}
