import { isSlug } from "@/shared/config/brands";

/** "aquafix, vifnet" → ["aquafix", "vifnet"]; null if any is not a brand slug or none is given. */
export function brandsFrom(raw: string): string[] | null {
  const brands = [...new Set(raw.split(/[\s,]+/).filter(Boolean))];
  return brands.length > 0 && brands.every(isSlug) ? brands : null;
}
