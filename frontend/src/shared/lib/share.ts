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

/** `panel_core::funnel::MIN_SAMPLE`: below this many in the denominator there is no percent. */
export const MIN_SAMPLE = 30;

/**
 * A share counted here, by the backend's rule (`Share::new`): a whole percent,
 * rounded half up, only from `minSample` on. For counts the API does not make yet.
 */
export function shareOf(n: number, of: number, minSample = MIN_SAMPLE): Share {
  const percent = of >= minSample && of > 0 ? Math.floor((n * 100 + Math.floor(of / 2)) / of) : null;
  return { n, of, percent, small_sample: percent === null };
}
