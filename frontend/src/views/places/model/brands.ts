import { brandsOf } from "@/entities/place";
import type { Place } from "@/entities/place";
import type { Source } from "@/entities/source";

/**
 * The brands a new place may be added under: every place's, and every brand an active
 * source writes for — a brand's first place has no place to name the brand yet.
 */
export function offeredBrands(places: readonly Pick<Place, "brand">[], sources: readonly Pick<Source, "brands" | "revoked_at">[]): string[] {
  const granted = sources.filter((s) => s.revoked_at === null).flatMap((s) => s.brands.map((brand) => ({ brand })));
  return brandsOf([...places, ...granted]);
}
