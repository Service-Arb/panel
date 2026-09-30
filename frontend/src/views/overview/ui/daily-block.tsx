"use client";

import { Badge, Card, CardContent, CardHeader, CardTitle } from "@evinvest/uikit";

import { useT } from "@/shared/i18n";
import { EmptyState } from "@/shared/ui/empty-state";

/**
 * Stages 1–4 (Maps and site, per day). There is no import yet (phase 2), so the
 * block says so instead of showing numbers — a zero would claim nobody came.
 */
export function DailyBlock() {
  const t = useT();
  return (
    <Card className="gap-3 py-4">
      <CardHeader className="px-4">
        <CardTitle className="text-xs font-medium uppercase tracking-wide text-ink-soft">{t("funnel.daily.title")}</CardTitle>
      </CardHeader>
      <CardContent className="px-4">
        <EmptyState className="p-6" title={t("funnel.daily.pendingTitle")} description={t("funnel.daily.pending")} />
      </CardContent>
    </Card>
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
