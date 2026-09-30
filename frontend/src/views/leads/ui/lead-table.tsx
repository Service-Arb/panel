"use client";

import { Badge, Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@evinvest/uikit";

import { type Lead, SlaBadge, StageBadge, contactOf } from "@/entities/lead";
import { useLocale, useT } from "@/shared/i18n";
import { formatDateTime } from "@/shared/lib/format";

/** Desktop: the queue as a table; a row opens the lead beside it. */
export function LeadTable({ leads, onOpen }: { leads: Lead[]; onOpen: (lead: Lead) => void }) {
  const t = useT();
  const locale = useLocale();
  return (
    <Table>
      <TableHeader>
        <TableRow>
          <TableHead>{t("leads.col.need")}</TableHead>
          <TableHead>{t("leads.col.where")}</TableHead>
          <TableHead>{t("leads.col.stage")}</TableHead>
          <TableHead>{t("leads.col.waiting")}</TableHead>
          <TableHead className="text-right">{t("leads.col.created")}</TableHead>
        </TableRow>
      </TableHeader>
      <TableBody>
        {leads.map((lead) => {
          const c = contactOf(lead.pii);
          return (
            <TableRow key={`${lead.brand}/${lead.lead_id}`} className="cursor-pointer" onClick={() => onOpen(lead)}>
              <TableCell className="max-w-72">
                {/* The row takes the click; the button is its keyboard stop. */}
                <button type="button" className="block w-full truncate rounded-sm text-left text-ink outline-none focus-visible:ring-2 focus-visible:ring-ring">
                  {c.need ?? c.name ?? t("leads.noNeed")}
                </button>
              </TableCell>
              <TableCell className="text-ink-mid">
                {lead.brand} · {lead.location ?? "—"} {lead.manual && <Badge variant="outline">{t("leads.manual")}</Badge>}
              </TableCell>
              <TableCell>
                <StageBadge stage={lead.stage} />
              </TableCell>
              <TableCell>
                <SlaBadge sla={lead.sla} />
              </TableCell>
              <TableCell className="text-right tabular-nums text-ink-soft">{lead.created_at ? formatDateTime(lead.created_at, locale) : "—"}</TableCell>
            </TableRow>
          );
        })}
      </TableBody>
    </Table>
  );
}
