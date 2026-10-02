"use client";

import { Alert, AlertDescription, Badge, Skeleton, Tabs, TabsContent, TabsList, TabsTrigger } from "@evinvest/uikit";
import { useState } from "react";

import { type PlaceKey, type PlaceSettingsView, fetchPlaceSettings } from "@/entities/place";
import { managesPlaces, useMe } from "@/entities/session";
import { SettingsForm, SettingsSummary } from "@/features/edit-place-settings";
import { WithdrawButton } from "@/features/withdraw-place";
import { useLocale, useT } from "@/shared/i18n";
import { formatDateTime } from "@/shared/lib/format";
import { useResource } from "@/shared/lib/use-resource";
import { ErrorState } from "@/shared/ui/error-state";

import { PlaceHistory } from "./place-history";

/**
 * A place's live site data: the form (or, for an operator, what it says), the
 * history with reverts, and for an admin the switch that withdraws the point.
 * Every write re-reads the place, so the form starts again from what is stored.
 */
export function PlaceSettingsPanel({ place, onChanged }: { place: PlaceKey; onChanged: () => void }) {
  const t = useT();
  const { role } = useMe();
  const [version, setVersion] = useState(0);
  const [saved, setSaved] = useState(false);
  const id = `${place.brand}/${place.slug}`;
  const settings = useResource(`settings:${id}:${version}`, () => fetchPlaceSettings(place), `settings:${id}`);

  if (settings.status === "loading") return <Skeleton className="h-64 w-full" />;
  if (settings.status === "error") return <ErrorState failure={settings.failure} onRetry={settings.reload} />;

  const view = settings.data;
  const changed = (note: boolean) => () => {
    setSaved(note);
    setVersion((v) => v + 1);
    onChanged();
  };
  const written = changed(true);

  return (
    <div className="flex flex-col gap-4">
      <PlaceStatus view={view} />
      {saved && (
        <Alert variant="success">
          <AlertDescription>{t("placeSettings.saved")}</AlertDescription>
        </Alert>
      )}
      <Tabs defaultValue="edit">
        <TabsList>
          <TabsTrigger value="edit">{t("placeSettings.tab.edit")}</TabsTrigger>
          <TabsTrigger value="history">{t("placeSettings.tab.history")}</TabsTrigger>
        </TabsList>
        <TabsContent value="edit" className="pt-3">
          {view.can_edit ? <SettingsForm key={view.updated_at ?? "never"} place={view} onSaved={written} onReload={changed(false)} /> : <SettingsSummary place={view} />}
        </TabsContent>
        <TabsContent value="history" className="pt-3">
          <PlaceHistory place={place} version={version} canEdit={view.can_edit} onChanged={written} />
        </TabsContent>
      </Tabs>
      {managesPlaces(role) && (
        // Last and apart: taking a point off the site is rare and not part of editing it.
        <div className="flex flex-col items-start gap-2 border-t border-border pt-4">
          <WithdrawButton place={view} onChanged={changed(false)} />
        </div>
      )}
    </div>
  );
}

function PlaceStatus({ view }: { view: PlaceSettingsView }) {
  const t = useT();
  const locale = useLocale();
  return (
    <div className="flex flex-col gap-2 text-sm">
      {view.withdrawn && (
        <div className="flex flex-wrap items-center gap-2">
          <Badge variant="destructive">{t("placeSettings.badge.withdrawn")}</Badge>
          <span className="text-ink-mid">{t("placeSettings.withdrawn.body")}</span>
        </div>
      )}
      <p className="text-ink-soft">
        {view.updated_at ? t("placeSettings.updated", { at: formatDateTime(view.updated_at, locale), by: view.updated_by ?? "—" }) : t("placeSettings.never")}
      </p>
    </div>
  );
}
