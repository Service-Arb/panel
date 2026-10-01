"use client";

import { Badge } from "@evinvest/uikit";

import type { Funnel } from "@/entities/funnel";
import { useT } from "@/shared/i18n";

import { intentToLead } from "../model/site";

/**
 * The seam between per-day aggregates and per-lead counts: nothing across it is
 * exact (§10.1). With intents counted, the step across it is given as an
 * estimate, and only as the two counts — never a percent.
 */
export function EstimateEdge({ funnel }: { funnel: Funnel }) {
  const t = useT();
  const leads = funnel.stages[0]?.reached ?? 0;
  const step = funnel.aggregate_source.imported_at === null ? null : intentToLead(leads, funnel.aggregate.intents.total);
  return (
    <div className="flex flex-col gap-1 px-1 text-sm text-ink-soft" role="note">
      <div className="flex flex-wrap items-center gap-x-3 gap-y-1">
        <Badge variant="outline">{t("funnel.edge.estimate")}</Badge>
        {step && (
          <span className="tabular-nums text-ink-mid">
            {t("funnel.edge.stepCounts", { leads: step.leads, intents: step.intents })}
          </span>
        )}
      </div>
      <span>{t("funnel.edge.body")}</span>
    </div>
  );
}
