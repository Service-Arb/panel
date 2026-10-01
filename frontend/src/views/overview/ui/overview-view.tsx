"use client";

import { Skeleton } from "@evinvest/uikit";

import { fetchFunnel } from "@/entities/funnel";
import { brandsOf, usePlaces } from "@/entities/place";
import { FunnelFilters, useFilterParams } from "@/features/funnel-filters";
import { useT } from "@/shared/i18n";
import { useResource } from "@/shared/lib/use-resource";
import { ErrorState } from "@/shared/ui/error-state";
import { PageHeader } from "@/shared/ui/page-header";

import { DailyBlock } from "./daily-block";
import { EstimateEdge } from "./estimate-edge";
import { LeadsBlock } from "./leads-block";

export function OverviewView() {
  const t = useT();
  const { period, brand, range, update } = useFilterParams();
  const places = usePlaces();
  const funnel = useResource(`funnel:${range.from}:${range.to}:${brand ?? ""}`, () => fetchFunnel({ ...range, brand }));

  return (
    <div className="flex flex-col gap-4 p-4 md:p-6">
      <PageHeader title={t("nav.overview")}>
        <FunnelFilters period={period} brand={brand} brands={brandsOf(places, brand)} onChange={update} />
      </PageHeader>
      <div className="flex max-w-3xl flex-col gap-3">
        {funnel.status === "loading" && <Skeleton className="h-96 w-full" />}
        {funnel.status === "error" && <ErrorState failure={funnel.failure} onRetry={funnel.reload} />}
        {funnel.status === "ok" && <DailyBlock funnel={funnel.data} />}
        {funnel.status === "ok" && <EstimateEdge funnel={funnel.data} />}
        {funnel.status === "ok" && <LeadsBlock funnel={funnel.data} />}
        {funnel.status === "ok" && <p className="px-1 text-xs text-ink-soft">{t("funnel.range", { from: funnel.data.from, to: funnel.data.to })}</p>}
      </div>
    </div>
  );
}
