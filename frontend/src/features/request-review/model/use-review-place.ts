"use client";

import { fetchPlaceSettings, followsPlace } from "@/entities/place";
import { useResource } from "@/shared/lib/use-resource";

export interface ReviewPlace {
  reviewUrl: string | null;
  brandName: string | null;
}

const NONE: ReviewPlace = { reviewUrl: null, brandName: null };

/**
 * What the message needs from the place's live settings: its Google review link
 * and the brand's name. Both null while loading or when the settings cannot be
 * read (no button then).
 */
export function useReviewPlace(brand: string, slug: string): ReviewPlace {
  const key = `review-place:${brand}/${slug}`;
  const place = useResource(key, () => fetchPlaceSettings({ brand, slug }), key, { live: followsPlace({ brand, slug }) });
  if (place.status !== "ok") return NONE;
  const { reviewUrl, brandName } = place.data.settings.edited;
  return { reviewUrl: reviewUrl ?? null, brandName: brandName ?? null };
}
