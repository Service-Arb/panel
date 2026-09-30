import { type Infer, arrayOf, nullable, object, oneOf, str } from "@/shared/lib/parse";

/** `panel_core::event::SourceKind`. */
export const SOURCE_KINDS = ["site", "review_archive", "gbp", "posthog", "panel", "sheet", "telephony"] as const;
export type SourceKind = (typeof SOURCE_KINDS)[number];

export const sourceParser = object({
  key_id: str,
  kind: oneOf(SOURCE_KINDS),
  brands: arrayOf(str),
  created_at: str,
  revoked_at: nullable(str),
});
export type Source = Infer<typeof sourceParser>;

export const sourcesParser = object({ sources: arrayOf(sourceParser) });
export const addedSourceParser = object({ key_id: str, secret: str });
export type AddedSource = Infer<typeof addedSourceParser>;
