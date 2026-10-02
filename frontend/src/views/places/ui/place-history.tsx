"use client";

import { Skeleton } from "@evinvest/uikit";

import { ChangeEntry, type PlaceKey, type PlaceSettingsView, fetchSettingsHistory } from "@/entities/place";
import { RevertButton } from "@/features/revert-place-change";
import { useT } from "@/shared/i18n";
import { useResource } from "@/shared/lib/use-resource";
import { EmptyState } from "@/shared/ui/empty-state";
import { ErrorState } from "@/shared/ui/error-state";

/** The place's changes, newest first, each with a revert for someone who may edit. */
export function PlaceHistory({ place, version, canEdit, onChanged }: { place: PlaceKey; version: number; canEdit: boolean; onChanged: (view: PlaceSettingsView) => void }) {
  const t = useT();
  const id = `${place.brand}/${place.slug}`;
  const history = useResource(`history:${id}:${version}`, () => fetchSettingsHistory(place), `history:${id}`);
  if (history.status === "loading") return <Skeleton className="h-32 w-full" />;
  if (history.status === "error") return <ErrorState failure={history.failure} onRetry={history.reload} />;
  if (history.data.length === 0) return <EmptyState className="p-4" title={t("placeSettings.history.empty")} />;
  return (
    <ul className="flex flex-col">
      {history.data.map((change) => (
        <ChangeEntry key={change.id} change={change} action={canEdit ? <RevertButton place={place} changeId={change.id} onReverted={onChanged} /> : undefined} />
      ))}
    </ul>
  );
}
