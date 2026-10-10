"use client";

import { fetchPlaceSettings, followsPlace } from "@/entities/place";
import { useResource } from "@/shared/lib/use-resource";

/**
 * The place's Google review link, from its live settings; null while loading,
 * when the place has none, or when its settings cannot be read (no button then).
 */
export function useReviewUrl(brand: string, slug: string): string | null {
  const key = `review-url:${brand}/${slug}`;
  const place = useResource(key, () => fetchPlaceSettings({ brand, slug }), key, { live: followsPlace({ brand, slug }) });
  return place.status === "ok" ? (place.data.settings.edited.reviewUrl ?? null) : null;
}
