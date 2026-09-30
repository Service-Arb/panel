"use client";

import { Badge } from "@evinvest/uikit";

import { type Lead, SlaBadge, StageBadge, contactOf } from "@/entities/lead";
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
  const shown = rows.filter(([, v]) => v !== null);

  return (
    <div className="flex flex-col gap-3">
      <div className="flex flex-wrap items-center gap-2">
        <StageBadge stage={lead.stage} />
        <SlaBadge sla={lead.sla} />
        {lead.manual && <Badge variant="outline">{t("leads.manual")}</Badge>}
        <span className="text-sm text-ink-soft">
          {lead.brand} · {lead.location ?? t("places.unknown")}
        </span>
      </div>
      {lead.lost_reason && <p className="text-sm text-ink-mid">{t("card.lostReason", { reason: lead.lost_reason })}</p>}
      {shown.length === 0 ? (
        <p className="text-sm text-ink-soft">{t("card.noPii")}</p>
      ) : (
        <dl className="grid grid-cols-[auto_1fr] gap-x-4 gap-y-1 text-sm">
          {shown.map(([key, value]) => (
            <div key={key} className="contents">
              <dt className="text-ink-soft">{t(key)}</dt>
              <dd className="break-words text-ink">{value}</dd>
            </div>
          ))}
        </dl>
      )}
    </div>
  );
}
