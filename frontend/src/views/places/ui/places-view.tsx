"use client";

import { Empty, EmptyDescription, EmptyHeader, EmptyTitle, Skeleton } from "@evinvest/uikit";
import { useState } from "react";

import { fetchLeads } from "@/entities/lead";
import { useT } from "@/shared/i18n";
import { daysAgo } from "@/shared/lib/format";
import { useResource } from "@/shared/lib/use-resource";
import { ErrorState } from "@/shared/ui/error-state";
import { PageHeader } from "@/shared/ui/page-header";

import { PLACES_LIMIT, aggregatePlaces, loadWindow } from "../model/aggregate";
import { PlaceCard } from "./place-card";

const WINDOW_DAYS = 30;
const ALL = { stage: null, brand: null, location: null, overdue: false };

/**
 * Locations with a mini funnel each. The API has no per-location funnel yet, so
 * the counts are made here from the leads list — only while it is small; past
 * {@link PLACES_LIMIT} leads the screen says which endpoint is missing instead.
 */
export function PlacesView() {
  const t = useT();
  const [fromIso] = useState(() => daysAgo(new Date(), WINDOW_DAYS).toISOString());
  const data = useResource(`places:${fromIso}`, () => loadWindow((cursor, limit) => fetchLeads(ALL, cursor, limit), fromIso));

  return (
    <div className="flex flex-col gap-4 p-4 md:p-6">
      <PageHeader title={t("places.title")} />
      <p className="text-sm text-ink-soft">{t("places.window", { days: WINDOW_DAYS })}</p>
      {data.status === "loading" && <Skeleton className="h-48 w-full" />}
      {data.status === "error" && <ErrorState failure={data.failure} onRetry={data.reload} />}
      {data.status === "ok" && !data.data.complete && (
        <Empty>
          <EmptyHeader>
            <EmptyTitle>{t("places.tooMany.title")}</EmptyTitle>
            <EmptyDescription>{t("places.tooMany.body", { limit: PLACES_LIMIT })}</EmptyDescription>
          </EmptyHeader>
        </Empty>
      )}
      {data.status === "ok" && data.data.complete && <PlaceGrid rows={aggregatePlaces(data.data.leads, fromIso)} />}
    </div>
  );
}

function PlaceGrid({ rows }: { rows: ReturnType<typeof aggregatePlaces> }) {
  const t = useT();
  if (rows.length === 0) return <p className="text-sm text-ink-soft">{t("places.empty")}</p>;
  return (
    <div className="grid gap-3 sm:grid-cols-2 xl:grid-cols-3">
      {rows.map((row) => (
        <PlaceCard key={`${row.brand}/${row.location ?? ""}`} row={row} />
      ))}
    </div>
  );
}
