"use client";

import { Badge } from "@evinvest/uikit";

import type { LeadEvent } from "@/entities/lead";
import { useLocale, useT } from "@/shared/i18n";
import { formatDateTime } from "@/shared/lib/format";

/** The properties worth a glance, as `key: value`; ids and nested objects stay out. */
function summary(properties: Record<string, unknown>): string {
  return Object.entries(properties)
    .filter(([k, v]) => !k.endsWith("Id") && (typeof v === "string" || typeof v === "number" || typeof v === "boolean"))
    .map(([k, v]) => `${k}: ${String(v)}`)
    .join(" · ");
}

/** The journal of this lead, oldest first, the manual entries marked (§10a). */
export function EventList({ events }: { events: LeadEvent[] }) {
  const t = useT();
  const locale = useLocale();
  return (
    <ol className="flex flex-col gap-2">
      {events.map((e) => (
        <li key={e.id} className="flex flex-col gap-0.5 border-l-2 border-border pl-3 text-sm">
          <span className="flex flex-wrap items-center gap-2">
            <span className="font-mono text-xs text-ink">{e.type}</span>
            {e.manual && <Badge variant="outline">{t("leads.manual")}</Badge>}
            <span className="text-xs tabular-nums text-ink-soft">{formatDateTime(e.occurred_at, locale)}</span>
          </span>
          {summary(e.properties) && <span className="text-xs text-ink-mid">{summary(e.properties)}</span>}
        </li>
      ))}
    </ol>
  );
}
