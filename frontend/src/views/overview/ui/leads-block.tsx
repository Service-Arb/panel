"use client";

import { Badge, cn } from "@evinvest/uikit";

import { type Funnel, biggestLoss } from "@/entities/funnel";
import { useT } from "@/shared/i18n";
import { formatShare } from "@/shared/lib/share";

/**
 * Stages 5–10, counted per lead. Each step says how many of the step before it
 * reached it — as a percent only when the sample allows, otherwise "n of m" —
 * and the step that loses the most leads is marked.
 */
export function LeadsBlock({ funnel }: { funnel: Funnel }) {
  const t = useT();
  const worst = biggestLoss(funnel.stages);
  const leads = funnel.stages[0]?.reached ?? 0;

  return (
    <section className="flex flex-col gap-2 rounded-lg border border-border bg-card p-4">
      <h2 className="text-xs font-medium uppercase tracking-wide text-ink-soft">{t("funnel.leads.title")}</h2>
      {leads === 0 ? (
        <p className="py-2 text-sm text-ink-soft">{t("funnel.empty")}</p>
      ) : (
        <ol className="flex flex-col divide-y divide-border">
          {funnel.stages.map((step) => {
            const hot = step.stage === worst;
            return (
              <li key={step.stage} className={cn("grid grid-cols-[1fr_auto_minmax(7rem,auto)] items-center gap-3 py-2 text-sm", hot && "text-ink")}>
                <span className={cn(hot ? "font-medium text-ink" : "text-ink-mid")}>{t(`funnel.stage.${step.stage}`)}</span>
                <span className="text-right font-medium tabular-nums text-ink">{step.reached}</span>
                <span className="flex items-center justify-end gap-2 text-right tabular-nums text-ink-soft">
                  {step.of_previous && <span title={t("funnel.ofPrevious")}>{formatShare(step.of_previous, t)}</span>}
                  {hot && step.of_previous && (
                    <Badge variant="destructive">
                      {t("funnel.biggestLoss")} {t("funnel.lostCount", { n: step.of_previous.of - step.reached })}
                    </Badge>
                  )}
                </span>
              </li>
            );
          })}
        </ol>
      )}
      {leads > 0 && (
        <div className="flex flex-wrap gap-x-6 gap-y-1 pt-1 text-sm text-ink-soft">
          <span>{t("funnel.lost", { share: formatShare(funnel.lost, t) })}</span>
          <span>{t("funnel.manual", { share: formatShare(funnel.manual, t) })}</span>
        </div>
      )}
    </section>
  );
}
