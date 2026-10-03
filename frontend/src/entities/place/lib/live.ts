import type { ChangedEvent } from "@/shared/lib/live";

import type { PlaceKey } from "../api/settings";
import type { PlaceSettingsView } from "../model/settings";

/** A `places` change on this place — or on any place, when the event names none. */
export function followsPlace(key: PlaceKey): (event: ChangedEvent) => boolean {
  return (e) => e.topic === "places" && (e.brand_id === null || e.brand_id === key.brand) && (e.id === null || e.id === key.slug);
}

const timeOf = (at: string | null) => (at === null ? Number.NEGATIVE_INFINITY : Date.parse(at));

/**
 * Whether `read` holds a save made after `base` was read. Strictly newer: the
 * re-read after one's own save, or an older answer still on screen while it
 * lands, is not someone else's change.
 */
export function savedSince(read: Pick<PlaceSettingsView, "updated_at">, base: Pick<PlaceSettingsView, "updated_at">): boolean {
  return timeOf(read.updated_at) > timeOf(base.updated_at);
}
