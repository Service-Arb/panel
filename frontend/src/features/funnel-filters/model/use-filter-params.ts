"use client";

import { usePathname, useRouter, useSearchParams } from "next/navigation";
import { useState } from "react";

import { type Period, periodFrom, rangeOf } from "./period";

export type FilterPatch = { period?: Period; brand?: string | null };

/**
 * Period and brand kept in the query string, so a screen can be linked to as it
 * is cut. The range is fixed when the screen opens: one that moved mid-render
 * would refetch in a loop.
 */
export function useFilterParams() {
  const params = useSearchParams();
  const router = useRouter();
  const pathname = usePathname();
  const period = periodFrom(params.get("period"));
  const brand = params.get("brand") || null;
  const [now] = useState(() => new Date());
  const range = rangeOf(period, now);

  const update = (patch: FilterPatch) => {
    const next = new URLSearchParams(params);
    if (patch.period !== undefined) next.set("period", String(patch.period));
    if (patch.brand) next.set("brand", patch.brand);
    else if (patch.brand === null) next.delete("brand");
    router.replace(`${pathname}?${next.toString()}`);
  };

  return { period, brand, range, update };
}
