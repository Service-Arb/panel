"use client";

import { Skeleton } from "@evinvest/uikit";
import { useState } from "react";

import { fetchFunnel } from "@/entities/funnel";
import { fetchLeads } from "@/entities/lead";
import { useT } from "@/shared/i18n";
import { daysAgo, utcDay } from "@/shared/lib/format";
import { useResource } from "@/shared/lib/use-resource";
import { EmptyState } from "@/shared/ui/empty-state";
import { ErrorState } from "@/shared/ui/error-state";
import { PageHeader } from "@/shared/ui/page-header";

import { PLACES_LIMIT, type PlaceRow, aggregatePlaces, loadWindow } from "../model/aggregate";
import { PlaceCard } from "./place-card";

const WINDOW_DAYS = 30;
const ALL = { stage: null, brand: null, location: null, overdue: false };

/**
 * The leads of the window, and the minimum sample from `/funnel` — the backend
 * decides below how many a share is "n of m", the browser does not.
 */
async function load(from: Date, now: Date) {
  const [window, funnel] = await Promise.all([
    loadWindow((cursor, limit) => fetchLeads(ALL, cursor, limit), from.toISOString()),
    fetchFunnel({ from: utcDay(from), to: utcDay(now), brand: null }),
  ]);
  return { ...window, rows: aggregatePlaces(window.leads, from.toISOString()), minSample: funnel.min_sample };
}

/**
 * Locations with a mini funnel each. The API has no per-location funnel yet, so
 * the counts are made here from the leads list — only while it is small; past
 * {@link PLACES_LIMIT} leads the screen says which endpoint is missing instead.
 */
export function PlacesView() {
  const t = useT();
  const [range] = useState(() => {
    const now = new Date();
    return { now, from: daysAgo(now, WINDOW_DAYS) };
  });
  const data = useResource(`places:${range.from.toISOString()}`, () => load(range.from, range.now));

  return (
    <div className="flex flex-col gap-4 p-4 md:p-6">
      <PageHeader title={t("places.title")} />
      <p className="text-sm text-ink-soft">{t("places.window", { days: WINDOW_DAYS })}</p>
      {data.status === "loading" && <Skeleton className="h-48 w-full" />}
      {data.status === "error" && <ErrorState failure={data.failure} onRetry={data.reload} />}
      {data.status === "ok" && !data.data.complete && <EmptyState title={t("places.tooMany.title")} description={t("places.tooMany.body", { limit: PLACES_LIMIT })} />}
      {data.status === "ok" && data.data.complete && <PlaceGrid rows={data.data.rows} minSample={data.data.minSample} />}
    </div>
  );
}

function PlaceGrid({ rows, minSample }: { rows: PlaceRow[]; minSample: number }) {
  const t = useT();
  if (rows.length === 0) return <EmptyState title={t("places.empty")} description={t("places.empty.body")} />;
  return (
    <div className="grid gap-3 sm:grid-cols-2 xl:grid-cols-3">
      {rows.map((row) => (
        <PlaceCard key={`${row.brand}/${row.location ?? ""}`} row={row} minSample={minSample} />
      ))}
    </div>
  );
}
