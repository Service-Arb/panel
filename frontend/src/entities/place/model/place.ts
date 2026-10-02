import { type Infer, type Parser, arrayOf, bool, nullable, object, str } from "@/shared/lib/parse";

/** Absent reads as false: a backend from before place settings has no such flag. */
const flag: Parser<boolean> = (v, path) => (v === undefined || v === null ? false : bool(v, path));

const placeParser = object({ brand: str, location: str, last_lead_at: nullable(str), has_settings: flag, withdrawn: flag });
export type Place = Infer<typeof placeParser>;

/**
 * `GET /places`: every location a lead or a visit names, and those added by
 * hand, so the filters offer what exists; each says whether it has live site
 * data and whether it was withdrawn.
 */
export const placesParser = object({ places: arrayOf(placeParser) });

const uniq = (xs: Iterable<string>) => [...new Set(xs)].sort();

/** The brands to offer, plus the one already chosen even if no place names it. */
export function brandsOf(places: readonly Pick<Place, "brand">[], chosen: string | null = null): string[] {
  return uniq([...places.map((p) => p.brand), ...(chosen ? [chosen] : [])]);
}

/** A brand's locations (every brand's when none is chosen), plus the chosen one. */
export function locationsOf(places: readonly Pick<Place, "brand" | "location">[], brand: string | null, chosen: string | null = null): string[] {
  return uniq([...places.filter((p) => !brand || p.brand === brand).map((p) => p.location), ...(chosen ? [chosen] : [])]);
}
