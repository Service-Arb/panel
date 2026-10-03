import { type Infer, type Parser, ParseError, arrayOf, bool, nullable, num, object, str } from "@/shared/lib/parse";

/**
 * Only http(s): the link is put in an `href`, and a `javascript:` one from a
 * misconfigured backend must not become a click away from running.
 */
const webUrl: Parser<string> = (v, path) => {
  const s = str(v, path);
  let url: URL;
  try {
    url = new URL(s);
  } catch {
    throw new ParseError(`${path}: expected a URL, got ${JSON.stringify(s)}`);
  }
  if (url.protocol !== "https:" && url.protocol !== "http:") throw new ParseError(`${path}: expected an http(s) URL`);
  return s;
};

/** How a landing splits its traffic: one weight per variant (same order), off or on, and the share kept out of it. */
const settingsParser = object({ weights: arrayOf(num), enabled: bool, holdout: nullable(num) });
export type ExperimentSettings = Infer<typeof settingsParser>;

/** What the brand's landing declared in code at its last start. */
const declaredParser = object({
  weights: arrayOf(num),
  enabled: bool,
  holdout: nullable(num),
  summary: nullable(str),
  declared_at: str,
});

/** The operator's changes over the declaration; a null field follows the code. */
const overrideParser = object({
  weights: nullable(arrayOf(num)),
  enabled: nullable(bool),
  holdout: nullable(num),
  changed_by: str,
  changed_at: str,
});
export type ExperimentOverride = Infer<typeof overrideParser>;

/**
 * An experiment as config, not as numbers: the statistics are PostHog's, and
 * `posthog_url` opens them (null while the panel has no PostHog project). The
 * first variant is the control. A retired one is no longer declared and cannot
 * be changed.
 */
export const experimentParser = object({
  brand: str,
  key: str,
  variants: arrayOf(str),
  declared: declaredParser,
  override: nullable(overrideParser),
  effective: settingsParser,
  weights_changed_at: nullable(str),
  retired: bool,
  posthog_url: nullable(webUrl),
});
export type Experiment = Infer<typeof experimentParser>;

/** `GET /experiments`. */
export const experimentsParser = object({ experiments: arrayOf(experimentParser) });

/**
 * The body of `PUT /experiments/{brand}/{key}`: an absent field is left as it
 * is, a null one goes back to what the code declares.
 */
export interface ExperimentPatch {
  enabled?: boolean | null;
  weights?: number[] | null;
  holdout?: number | null;
}
