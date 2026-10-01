"use client";

import { Skeleton } from "@evinvest/uikit";
import { useState } from "react";

import { fetchFunnelByLocation } from "@/entities/funnel";
import { type Period, rangeOf } from "@/features/funnel-filters";
import { useT } from "@/shared/i18n";
import { useResource } from "@/shared/lib/use-resource";
import { EmptyState } from "@/shared/ui/empty-state";
import { ErrorState } from "@/shared/ui/error-state";
import { PageHeader } from "@/shared/ui/page-header";

import { type PlaceRow, placeRows } from "../model/rows";
import { PlaceCard } from "./place-card";

const WINDOW_DAYS: Period = 30;

/** Locations with a mini funnel each, from the backend's per-location funnel. */
export function PlacesView() {
  const t = useT();
  // Fixed when the screen opens: a range that moved mid-render would refetch in a loop.
  const [range] = useState(() => rangeOf(WINDOW_DAYS, new Date()));
  const data = useResource(`places:${range.from}:${range.to}`, async () => {
    const funnel = await fetchFunnelByLocation({ ...range, brand: null });
    return { rows: placeRows(funnel.locations), minSample: funnel.min_sample };
  });

  return (
    <div className="flex flex-col gap-4 p-4 md:p-6">
      <PageHeader title={t("places.title")} />
      {data.status === "loading" && <Skeleton className="h-48 w-full" />}
      {data.status === "error" && <ErrorState failure={data.failure} onRetry={data.reload} />}
      {data.status === "ok" && <p className="text-sm text-ink-soft">{t("places.window", { days: WINDOW_DAYS, min: data.data.minSample })}</p>}
      {data.status === "ok" && <PlaceGrid rows={data.data.rows} />}
    </div>
  );
}

function PlaceGrid({ rows }: { rows: PlaceRow[] }) {
  const t = useT();
  if (rows.length === 0) return <EmptyState title={t("places.empty")} description={t("places.empty.body")} />;
  return (
    <div className="grid gap-3 sm:grid-cols-2 xl:grid-cols-3">
      {rows.map((row) => (
        <PlaceCard key={`${row.brand}/${row.location ?? ""}`} row={row} />
      ))}
    </div>
  );
}
