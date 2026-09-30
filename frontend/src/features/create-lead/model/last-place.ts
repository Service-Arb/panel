/**
 * The location the operator typed a call for last (spec §10a: "the last one by
 * default"). A convenience of this browser only; losing it costs one tap.
 */
const KEY = "sa.panel.lastPlace";

export interface Place {
  brand: string;
  location: string;
}

export function readLastPlace(): Place | null {
  try {
    const raw = window.localStorage.getItem(KEY);
    if (!raw) return null;
    const v: unknown = JSON.parse(raw);
    if (typeof v === "object" && v !== null && "brand" in v && "location" in v && typeof v.brand === "string" && typeof v.location === "string") {
      return { brand: v.brand, location: v.location };
    }
  } catch {
    // Storage blocked or the value garbled: start from nothing.
  }
  return null;
}

export function writeLastPlace(place: Place): void {
  try {
    window.localStorage.setItem(KEY, JSON.stringify(place));
  } catch {
    // Not remembering is fine.
  }
}
