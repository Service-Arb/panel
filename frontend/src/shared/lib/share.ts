import type { T } from "@/shared/i18n";

/**
 * `n` out of `of` as the backend computes it (`panel_core::funnel::Share`):
 * `percent` is present only when `of` reached the minimum sample, and whole.
 */
export interface Share {
  n: number;
  of: number;
  percent: number | null;
  small_sample: boolean;
}

/**
 * The only way a share reaches the screen. A small sample is "12 of 17" and
 * never a percent — not even one computed here from `n` and `of` (spec §10.1):
 * the backend withholding it is the decision, not a gap to fill.
 */
export function formatShare(share: Share, t: T): string {
  if (share.small_sample || share.percent === null) return t("share.nOf", { n: share.n, of: share.of });
  return t("share.percent", { percent: share.percent });
}

