import { http, ignoreBody } from "@/shared/api";

import { type PlaceSettings, type PlaceSettingsView, type SettingsChange, historyParser, placeSettingsParser, settingsToWire } from "../model/settings";

/** A place as the settings routes name it: brand and slug, both path segments. */
export interface PlaceKey {
  brand: string;
  slug: string;
}

const placePath = ({ brand, slug }: PlaceKey) => `/api/v1/places/${encodeURIComponent(brand)}/${encodeURIComponent(slug)}`;

export function fetchPlaceSettings(key: PlaceKey): Promise<PlaceSettingsView> {
  return http.get(`${placePath(key)}/settings`, placeSettingsParser);
}

/**
 * A full replace. `expectedUpdatedAt` is the `updated_at` the form was read at;
 * a newer one on the server answers 409 rather than overwriting someone's change.
 */
export function savePlaceSettings(key: PlaceKey, settings: PlaceSettings, expectedUpdatedAt: string | null): Promise<PlaceSettingsView> {
  return http.send("PUT", `${placePath(key)}/settings`, { settings: settingsToWire(settings), expected_updated_at: expectedUpdatedAt }, placeSettingsParser);
}

export async function fetchSettingsHistory(key: PlaceKey): Promise<SettingsChange[]> {
  return (await http.get(`${placePath(key)}/settings/history`, historyParser)).changes;
}

/** The change's `before` becomes current; the revert is itself a change in the history. */
export function revertSettingsChange(key: PlaceKey, changeId: string): Promise<PlaceSettingsView> {
  return http.send("POST", `${placePath(key)}/settings/revert/${encodeURIComponent(changeId)}`, undefined, placeSettingsParser);
}

/** Admin only: the site answers 404 for a withdrawn place until it is restored. */
export function setPlaceWithdrawn(key: PlaceKey, withdrawn: boolean): Promise<PlaceSettingsView> {
  return http.send("POST", `${placePath(key)}/${withdrawn ? "withdraw" : "restore"}`, undefined, placeSettingsParser);
}

/** Admin only: a place no lead or visit has named yet. */
export async function addPlace(key: PlaceKey): Promise<void> {
  await http.send("POST", "/api/v1/places", key, ignoreBody);
}
