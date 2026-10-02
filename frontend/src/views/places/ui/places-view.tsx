"use client";

import { Skeleton } from "@evinvest/uikit";
import { useState } from "react";

import { fetchFunnelByLocation } from "@/entities/funnel";
import { type PlaceKey, brandsOf, fetchPlaces } from "@/entities/place";
import { managesPlaces, useMe } from "@/entities/session";
import { AddPlaceButton } from "@/features/add-place";
import { type Period, rangeOf } from "@/features/funnel-filters";
import { useT } from "@/shared/i18n";
import { useResource } from "@/shared/lib/use-resource";
import { EmptyState } from "@/shared/ui/empty-state";
import { ErrorState } from "@/shared/ui/error-state";
import { PageHeader } from "@/shared/ui/page-header";
import { PanelOverlay } from "@/shared/ui/panel-overlay";

import { type PlaceRow, placeRows } from "../model/rows";
import { PlaceCard } from "./place-card";
import { PlaceSettingsPanel } from "./place-settings-panel";

const WINDOW_DAYS: Period = 30;

/**
 * Locations with a mini funnel each, from the backend's per-location funnel,
 * plus what `/places` says of their site data; a card opens that data.
 */
export function PlacesView() {
  const t = useT();
  const { role } = useMe();
  // Fixed when the screen opens: a range that moved mid-render would refetch in a loop.
  const [range] = useState(() => rangeOf(WINDOW_DAYS, new Date()));
  const [version, setVersion] = useState(0);
  const [open, setOpen] = useState<PlaceKey | null>(null);
  const refresh = () => setVersion((v) => v + 1);
  const data = useResource(
    `places:${range.from}:${range.to}:${version}`,
    async () => {
      const [funnel, places] = await Promise.all([fetchFunnelByLocation({ ...range, brand: null }), fetchPlaces()]);
      return { rows: placeRows(funnel.locations, places), minSample: funnel.min_sample, brands: brandsOf(places) };
    },
    `places:${range.from}:${range.to}`,
  );

  return (
    <div className="flex flex-col gap-4 p-4 md:p-6">
      <PageHeader title={t("places.title")}>
        {managesPlaces(role) && data.status === "ok" && (
          <AddPlaceButton
            brands={data.data.brands}
            onAdded={(key) => {
              refresh();
              setOpen(key);
            }}
          />
        )}
      </PageHeader>
      {data.status === "loading" && <Skeleton className="h-48 w-full" />}
      {data.status === "error" && <ErrorState failure={data.failure} onRetry={data.reload} />}
      {data.status === "ok" && <p className="text-sm text-ink-soft">{t("places.window", { days: WINDOW_DAYS, min: data.data.minSample })}</p>}
      {data.status === "ok" && <PlaceGrid rows={data.data.rows} onOpen={setOpen} />}
      <PanelOverlay
        open={open !== null}
        onOpenChange={(o) => !o && setOpen(null)}
        desktop="sheet"
        title={open ? t("placeSettings.openFor", { brand: open.brand, slug: open.slug }) : ""}
        description={t("placeSettings.description")}
      >
        {open && <PlaceSettingsPanel key={`${open.brand}/${open.slug}`} place={open} onChanged={refresh} />}
      </PanelOverlay>
    </div>
  );
}

function PlaceGrid({ rows, onOpen }: { rows: PlaceRow[]; onOpen: (key: PlaceKey) => void }) {
  const t = useT();
  if (rows.length === 0) return <EmptyState title={t("places.empty")} description={t("places.empty.body")} />;
  return (
    <div className="grid gap-3 sm:grid-cols-2 xl:grid-cols-3">
      {rows.map((row) => {
        const slug = row.location;
        return <PlaceCard key={`${row.brand}/${slug ?? ""}`} row={row} onOpen={slug === null ? null : () => onOpen({ brand: row.brand, slug })} />;
      })}
    </div>
  );
}
