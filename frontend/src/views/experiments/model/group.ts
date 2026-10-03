import type { Experiment } from "@/entities/experiment";

/**
 * The experiments by brand, brands in name order; within a brand the declared
 * ones first in the backend's order, the retired after them.
 */
export function byBrand(experiments: readonly Experiment[]): [string, Experiment[]][] {
  const groups = new Map<string, Experiment[]>();
  for (const e of experiments) groups.set(e.brand, [...(groups.get(e.brand) ?? []), e]);
  return [...groups.entries()]
    .sort(([a], [b]) => a.localeCompare(b))
    .map(([brand, list]) => [brand, [...list.filter((e) => !e.retired), ...list.filter((e) => e.retired)]]);
}
