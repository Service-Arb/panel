"use client";

import { Badge } from "@evinvest/uikit";

import { type MessageKey, useT } from "@/shared/i18n";

const DAILY: readonly MessageKey[] = ["funnel.daily.impressions", "funnel.daily.actions", "funnel.daily.visits", "funnel.daily.intent"];

/**
 * Stages 1–4 (Maps and site, per day). There is no import yet (phase 2), so the
 * rows say so instead of showing numbers — a zero would claim nobody came.
 */
export function DailyBlock() {
  const t = useT();
  return (
    <section className="flex flex-col gap-2 rounded-lg border border-border bg-card p-4">
      <h2 className="text-xs font-medium uppercase tracking-wide text-ink-soft">{t("funnel.daily.title")}</h2>
      <ul className="flex flex-col divide-y divide-border">
        {DAILY.map((key) => (
          <li key={key} className="flex items-center justify-between py-2 text-sm text-ink-mid">
            <span>{t(key)}</span>
            <span aria-hidden className="text-ink-soft">
              —
            </span>
          </li>
        ))}
      </ul>
      <p className="text-sm text-ink-soft">{t("funnel.daily.pending")}</p>
    </section>
  );
}

/** The seam between per-day aggregates and per-lead counts: nothing across it is exact (§10.1). */
export function EstimateEdge() {
  const t = useT();
  return (
    <div className="flex items-center gap-3 px-1 text-sm text-ink-soft" role="note">
      <Badge variant="outline">{t("funnel.edge.estimate")}</Badge>
      <span>{t("funnel.edge.body")}</span>
    </div>
  );
}
