"use client";

import { Settled, Skeleton } from "@evinvest/uikit";

import { type Experiments, fetchExperiments } from "@/entities/experiment";
import { brandsOf, usePlaces } from "@/entities/place";
import { FunnelFilters, useFilterParams } from "@/features/funnel-filters";
import { ROUTES } from "@/shared/config/routes";
import { useT } from "@/shared/i18n";
import { useResource } from "@/shared/lib/use-resource";
import { EmptyState } from "@/shared/ui/empty-state";
import { ErrorState } from "@/shared/ui/error-state";
import { ScreenFrame } from "@/shared/ui/screen-frame";

import { ExperimentCard } from "./experiment-card";

/** The landings' A/B tests as PostHog counted them: each variant against its control, no verdicts. */
export function ExperimentsView() {
  const t = useT();
  const { period, brand, range, update } = useFilterParams();
  const places = usePlaces();
  const key = `experiments:${range.from}:${range.to}:${brand ?? ""}`;
  const data = useResource(key, () => fetchExperiments({ ...range, brand }), key, { live: ["experiments", "metrics"] });
  const seen = data.status === "ok" ? data.data.experiments : [];

  return (
    <ScreenFrame
      title={t("experiments.title")}
      back={ROUTES.more}
      actions={<FunnelFilters period={period} brand={brand} brands={brandsOf([...places, ...seen], brand)} onChange={update} />}
    >
      <Settled loading={data.status === "loading"} skeleton={<Skeleton className="h-64 w-full max-w-3xl" />} className="flex max-w-3xl flex-col gap-3">
        {data.status === "error" && <ErrorState failure={data.failure} onRetry={data.reload} />}
        {data.status === "ok" && <ExperimentList data={data.data} />}
      </Settled>
    </ScreenFrame>
  );
}

function ExperimentList({ data }: { data: Experiments }) {
  const t = useT();
  if (data.source.imported_at === null) return <EmptyState title={t("experiments.pendingTitle")} description={t("experiments.pending")} />;
  if (data.experiments.length === 0) return <EmptyState title={t("experiments.empty")} description={t("experiments.empty.body")} />;
  return (
    <>
      <p className="px-1 text-sm text-ink-soft">{t("experiments.how", { min: data.min_exposures, confidence: Math.round(data.confidence * 100) })}</p>
      {data.experiments.map((e) => (
        <ExperimentCard key={`${e.brand}/${e.experiment}`} experiment={e} />
      ))}
      <p className="px-1 text-xs text-ink-soft">{t("funnel.range", { from: data.from, to: data.to })}</p>
    </>
  );
}
