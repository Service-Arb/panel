import type { Experiment } from "../model/experiment";

/**
 * The rules a landing applies an override by (`@evinvest/experiments`
 * `applyOverrides`): weights of the same length as the variants, none below 0,
 * summing above 0. A weights override that breaks them is silently ignored
 * there, so the panel refuses it before it is sent.
 */
export function validWeights(weights: readonly number[], variants: number): boolean {
  return weights.length === variants && weights.every((w) => Number.isFinite(w) && w >= 0) && weights.reduce((a, b) => a + b, 0) > 0;
}

/** A holdout is a share of visitors kept out of the experiment: in [0, 1). */
export function validHoldout(holdout: number): boolean {
  return Number.isFinite(holdout) && holdout >= 0 && holdout < 1;
}

/** Each variant's share of the traffic in percent; all zero when the weights sum to nothing. */
export function sharesOf(weights: readonly number[]): number[] {
  const sum = weights.reduce((a, b) => a + b, 0);
  return weights.map((w) => (sum > 0 ? (w / sum) * 100 : 0));
}

export function sameWeights(a: readonly number[], b: readonly number[]): boolean {
  return a.length === b.length && a.every((w, i) => w === b[i]);
}

/** No holdout and a holdout of 0 keep the same visitors out: none. */
export function sameHoldout(a: number | null, b: number | null): boolean {
  return (a ?? 0) === (b ?? 0);
}

export type ExperimentStatus = "retired" | "off" | "holdout" | "running";

/** What a row says first; a holdout is only worth saying while the experiment runs. */
export function statusOf(e: Pick<Experiment, "retired" | "effective">): ExperimentStatus {
  if (e.retired) return "retired";
  if (!e.effective.enabled) return "off";
  return (e.effective.holdout ?? 0) > 0 ? "holdout" : "running";
}
