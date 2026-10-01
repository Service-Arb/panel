"use client";

import { Card, CardContent, CardHeader, CardTitle } from "@evinvest/uikit";

import { type Funnel, INTENT_CHANNELS } from "@/entities/funnel";
import { useLocale, useT } from "@/shared/i18n";
import { formatDateTime } from "@/shared/lib/format";
import { EmptyState } from "@/shared/ui/empty-state";

import { sourceBreakdown } from "../model/site";

/**
 * Stages 3–4 as PostHog counted them, per day: page views and intents to get in
 * touch. Until the import has run there are no counts — not zeros — and the
 * block says so. Maps (stages 1–2) waits for the GBP import either way.
 */
export function DailyBlock({ funnel }: { funnel: Funnel }) {
  const t = useT();
  const locale = useLocale();
  const importedAt = funnel.aggregate_source.imported_at;
  return (
    <Card className="gap-3 py-4">
      <CardHeader className="px-4">
        <CardTitle className="text-xs font-medium uppercase tracking-wide text-ink-soft">{t("funnel.daily.title")}</CardTitle>
      </CardHeader>
      <CardContent className="flex flex-col gap-2 px-4">
        {importedAt === null ? (
          <EmptyState className="p-6" title={t("funnel.daily.pendingTitle")} description={t("funnel.daily.pending")} />
        ) : (
          <>
            <SiteRows funnel={funnel} />
            <p className="text-xs text-ink-soft">{t("funnel.daily.imported", { at: formatDateTime(importedAt, locale) })}</p>
          </>
        )}
        <p className="text-xs text-ink-soft">{t("funnel.daily.mapsPending")}</p>
      </CardContent>
    </Card>
  );
}

function SiteRows({ funnel }: { funnel: Funnel }) {
  const t = useT();
  const { visits, intents } = funnel.aggregate;
  const sources = sourceBreakdown(visits.by_source);
  const channels = INTENT_CHANNELS.filter((c) => intents.by_channel[c] > 0).map((c) => t("funnel.daily.channelCount", { channel: t(`funnel.channel.${c}`), n: intents.by_channel[c] }));
  const sourceParts = sources ? [...sources.top.map((s) => `${s.source} ${s.n}`), ...(sources.others > 0 ? [t("funnel.daily.otherSources", { n: sources.others })] : [])] : [];

  return (
    <ol className="flex flex-col divide-y divide-border">
      <li className="grid grid-cols-(--grid-funnel-row) items-center gap-x-3 gap-y-1 py-2 text-sm">
        <span className="min-w-0 text-ink-mid">{t("funnel.daily.visits")}</span>
        <span className="text-right font-medium tabular-nums text-ink">{visits.total}</span>
        <span />
        {sourceParts.length > 0 && <Breakdown label={t("funnel.daily.bySource")} parts={sourceParts} />}
      </li>
      <li className="grid grid-cols-(--grid-funnel-row) items-center gap-x-3 gap-y-1 py-2 text-sm">
        <span className="min-w-0 text-ink-mid">{t("funnel.daily.intent")}</span>
        <span className="text-right font-medium tabular-nums text-ink">{intents.total}</span>
        <span />
        {channels.length > 0 && <Breakdown label={t("funnel.daily.byChannel")} parts={channels} />}
      </li>
    </ol>
  );
}

/** A count's parts on their own line under it: beside the number they overflow a 360px screen. No separators: a wrapped line would start with one. */
function Breakdown({ label, parts }: { label: string; parts: string[] }) {
  return (
    <ul className="col-span-full flex flex-wrap gap-x-3 text-xs tabular-nums text-ink-soft" aria-label={label}>
      {parts.map((part) => (
        <li key={part} className="whitespace-nowrap">
          {part}
        </li>
      ))}
    </ul>
  );
}
