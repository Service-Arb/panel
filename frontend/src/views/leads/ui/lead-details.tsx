"use client";

import { Badge } from "@evinvest/uikit";

import { type Lead, SlaBadge, StageBadge, SuspectBadge, contactOf, extrasOf } from "@/entities/lead";
import { lostReasonLabel } from "@/features/move-stage";
import { useT } from "@/shared/i18n";

/** Who and what: the customer as they left it, where, and how long they have waited. */
export function LeadDetails({ lead }: { lead: Lead }) {
  const t = useT();
  const c = contactOf(lead.pii);
  const rows = [
    ["card.need", c.need],
    ["card.name", c.name],
    ["card.phone", c.phone],
    ["card.email", c.email],
  ] as const;
  const extras = extrasOf(lead.pii);
  // Text only, as React renders it: every value here is the customer's own input.
  const shown: { label: string; value: string; mono?: boolean }[] = [
    ...rows.flatMap(([key, value]) => (value === null ? [] : [{ label: t(key), value }])),
    ...extras.labelled.map(({ key, value }) => ({ label: t(`card.pii.${key}`), value })),
    ...extras.other.map(({ key, value }) => ({ label: key, value, mono: true })),
  ];

  return (
    <div className="flex flex-col gap-3">
      <div className="flex flex-wrap items-center gap-2">
        <StageBadge stage={lead.stage} />
        <SlaBadge sla={lead.sla} />
        <SuspectBadge suspect={lead.suspect} />
        {lead.manual && <Badge variant="outline">{t("leads.manual")}</Badge>}
        <span className="text-sm text-ink-soft">
          {lead.brand} · {lead.location ?? t("places.unknown")}
        </span>
      </div>
      {lead.lost_reason && <p className="text-sm text-ink-mid">{t("card.lostReason", { reason: lostReasonLabel(lead.lost_reason, t) })}</p>}
      {shown.length === 0 ? (
        <p className="text-sm text-ink-soft">{t("card.noPii")}</p>
      ) : (
        <dl className="grid grid-cols-[auto_1fr] gap-x-4 gap-y-1 text-sm">
          {shown.map(({ label, value, mono }) => (
            <div key={label} className="contents">
              <dt className={mono ? "font-mono text-xs text-ink-soft" : "text-ink-soft"}>{label}</dt>
              <dd className="min-w-0 wrap-anywhere text-ink">{value}</dd>
            </div>
          ))}
        </dl>
      )}
    </div>
  );
}
