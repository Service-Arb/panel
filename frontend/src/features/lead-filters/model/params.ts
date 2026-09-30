import { type LeadFilter, STAGES, type Stage } from "@/entities/lead";

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
