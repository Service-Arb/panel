import { type LeadFilter, STAGES, type Stage } from "@/entities/lead";
import type { MessageKey } from "@/shared/i18n";

/** The filter lives in the URL, so a view can be linked and survives a reload. */
export function leadFilterFrom(params: URLSearchParams): LeadFilter {
  const stage = params.get("stage");
  return {
    stage: STAGES.find((s) => s === stage) ?? null,
    brand: params.get("brand") || null,
    location: params.get("location") || null,
    overdue: params.get("overdue") === "1",
  };
}

export function paramsWith(params: URLSearchParams, patch: Partial<LeadFilter>): URLSearchParams {
  const next = new URLSearchParams(params);
  const set = (k: string, v: string | null) => (v ? next.set(k, v) : next.delete(k));
  if ("stage" in patch) set("stage", patch.stage ?? null);
  if ("brand" in patch) set("brand", patch.brand ?? null);
  if ("location" in patch) set("location", patch.location ?? null);
  if ("overdue" in patch) set("overdue", patch.overdue ? "1" : null);
  return next;
}

export function stageOrNull(v: string | null): Stage | null {
  return STAGES.find((s) => s === v) ?? null;
}

/**
 * The phone's stage segments (the mockup's "New · In progress · Quotes"): the
 * three stages that wait on the operator. The API filters by one stage, so
 * "in progress" is `contacted`.
 */
export const SEGMENTS: readonly { stage: Stage; key: MessageKey }[] = [
  { stage: "created", key: "filter.segment.new" },
  { stage: "contacted", key: "filter.segment.inProgress" },
  { stage: "quoted", key: "filter.segment.quotes" },
];
