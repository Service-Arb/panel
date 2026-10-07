import type { T } from "@/shared/i18n";

import type { SourceKind } from "../model/source";

/**
 * A kind as an admin reads it. Most are their wire names (`site`, `gbp`), which
 * is also what `panel source add --kind` takes; `bot` says which bots it means.
 */
export function sourceKindLabel(kind: SourceKind, t: T): string {
  return kind === "bot" ? t("sources.kind.bot") : kind;
}
