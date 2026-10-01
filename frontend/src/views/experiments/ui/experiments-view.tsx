"use client";

import { Skeleton } from "@evinvest/uikit";

import { type Experiments, fetchExperiments } from "@/entities/experiment";
import { brandsOf, usePlaces } from "@/entities/place";
import { FunnelFilters, useFilterParams } from "@/features/funnel-filters";
import { useT } from "@/shared/i18n";
import { useResource } from "@/shared/lib/use-resource";
import { EmptyState } from "@/shared/ui/empty-state";
import { ErrorState } from "@/shared/ui/error-state";
import { PageHeader } from "@/shared/ui/page-header";

import { ExperimentCard } from "./experiment-card";

/** The landings' A/B tests as PostHog counted them: each variant against its control, no verdicts. */
export function ExperimentsView() {
  const t = useT();
  const { period, brand, range, update } = useFilterParams();
  const places = usePlaces();
  const data = useResource(`experiments:${range.from}:${range.to}:${brand ?? ""}`, () => fetchExperiments({ ...range, brand }));
  const seen = data.status === "ok" ? data.data.experiments : [];

  return (
    <div className="flex flex-col gap-4 p-4 md:p-6">
      <PageHeader title={t("experiments.title")}>
        <FunnelFilters period={period} brand={brand} brands={brandsOf([...places, ...seen], brand)} onChange={update} />
      </PageHeader>
      <div className="flex max-w-3xl flex-col gap-3">
        {data.status === "loading" && <Skeleton className="h-64 w-full" />}
        {data.status === "error" && <ErrorState failure={data.failure} onRetry={data.reload} />}
        {data.status === "ok" && <ExperimentList data={data.data} />}
      </div>
    </div>
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
