import { hundredthsText } from "@/entities/pricing";
import type { T } from "@/shared/i18n";

import type { Shown } from "../model/errors";

/** A problem in the reader's words; the server's own words as they came. */
export function shownText(shown: Shown, t: T): string {
  if ("text" in shown) return shown.text;
  const { code, vars } = shown;
  // Every number the model bounds is in hundredths (cents, basis points): told as the editor types it.
  if (code === "int") return t("pricing.problem.int", { min: hundredthsText(Number(vars.min)), max: hundredthsText(Number(vars.max)) });
  return t(`pricing.problem.${code}`, { ...vars });
}
