/**
 * The brands of the vertical today. The API has no list of brands or locations
 * yet, so the filters start from this and add whatever the leads carry; a
 * brand missing here still shows up as soon as it has a lead.
 */
export const KNOWN_BRANDS: readonly string[] = ["aquafix", "vifnet"];

/** Lowercase slug of 1–64 of [a-z0-9_-], as `panel_core::ids::is_slug` takes it. */
export function isSlug(s: string): boolean {
  return /^[a-z0-9][a-z0-9_-]{0,63}$/.test(s);
}
