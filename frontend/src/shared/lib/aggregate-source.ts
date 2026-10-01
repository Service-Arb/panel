import { type Infer, nullable, object, oneOf, str } from "./parse";

/**
 * Where the per-day counts come from (`aggregate_source` of `/funnel`, `source` of
 * `/experiments`). `imported_at` is null until the PostHog import has run once: the
 * counts are then not zero but unknown, and the screens say so.
 */
export const aggregateSourceParser = object({
  source: oneOf(["posthog"]),
  kind: oneOf(["aggregate"]),
  imported_at: nullable(str),
});
export type AggregateSource = Infer<typeof aggregateSourceParser>;
