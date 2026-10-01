"use client";

import { Skeleton } from "@evinvest/uikit";
import { usePathname, useRouter, useSearchParams } from "next/navigation";
import { useState } from "react";

import { fetchFunnel } from "@/entities/funnel";
import { brandsOf, usePlaces } from "@/entities/place";
import { FunnelFilters, type Period, periodFrom, rangeOf } from "@/features/funnel-filters";
import { useT } from "@/shared/i18n";
import { useResource } from "@/shared/lib/use-resource";
import { ErrorState } from "@/shared/ui/error-state";
import { PageHeader } from "@/shared/ui/page-header";

import { DailyBlock, EstimateEdge } from "./daily-block";
import { LeadsBlock } from "./leads-block";

export function OverviewView() {
  const t = useT();
  const params = useSearchParams();
  const router = useRouter();
  const pathname = usePathname();
  const period = periodFrom(params.get("period"));
  const brand = params.get("brand") || null;
  // Fixed when the screen opens: a range that moved mid-render would refetch in a loop.
  const [now] = useState(() => new Date());
  const range = rangeOf(period, now);
  const places = usePlaces();
  const funnel = useResource(`funnel:${range.from}:${range.to}:${brand ?? ""}`, () => fetchFunnel({ ...range, brand }));

  const update = (patch: { period?: Period; brand?: string | null }) => {
    const next = new URLSearchParams(params);
    if (patch.period !== undefined) next.set("period", String(patch.period));
    if (patch.brand) next.set("brand", patch.brand);
    else if (patch.brand === null) next.delete("brand");
    router.replace(`${pathname}?${next.toString()}`);
  };

  return (
    <div className="flex flex-col gap-4 p-4 md:p-6">
      <PageHeader title={t("nav.overview")}>
        <FunnelFilters period={period} brand={brand} brands={brandsOf(places, brand)} onChange={update} />
      </PageHeader>
      <div className="flex max-w-3xl flex-col gap-3">
        <DailyBlock />
        <EstimateEdge />
        {funnel.status === "loading" && <Skeleton className="h-64 w-full" />}
        {funnel.status === "error" && <ErrorState failure={funnel.failure} onRetry={funnel.reload} />}
        {funnel.status === "ok" && <LeadsBlock funnel={funnel.data} />}
        {funnel.status === "ok" && <p className="px-1 text-xs text-ink-soft">{t("funnel.range", { from: funnel.data.from, to: funnel.data.to })}</p>}
      </div>
    </div>
  );
}
