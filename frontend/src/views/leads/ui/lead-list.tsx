"use client";

import { Item, ItemActions, ItemContent, ItemDescription, ItemGroup, ItemTitle } from "@evinvest/uikit";

import { type Lead, SlaBadge, StageBadge, contactOf } from "@/entities/lead";
import { useT } from "@/shared/i18n";

/** Phone: the queue as a list; a tap opens the lead in a bottom sheet. */
export function LeadList({ leads, onOpen }: { leads: Lead[]; onOpen: (lead: Lead) => void }) {
  const t = useT();
  return (
    <ItemGroup className="gap-2">
      {leads.map((lead) => {
        const c = contactOf(lead.pii);
        return (
          <Item key={`${lead.brand}/${lead.lead_id}`} variant="outline" size="sm" asChild>
            <button type="button" className="w-full text-left" onClick={() => onOpen(lead)}>
              <ItemContent className="min-w-0">
                <ItemTitle className="w-full truncate">{c.need ?? c.name ?? t("leads.noNeed")}</ItemTitle>
                <ItemDescription>
                  {lead.brand} · {lead.location ?? "—"}
                  {lead.manual && ` · ${t("leads.manual")}`}
                </ItemDescription>
              </ItemContent>
              <ItemActions className="flex-col items-end gap-1">
                {lead.sla ? <SlaBadge sla={lead.sla} /> : <StageBadge stage={lead.stage} />}
              </ItemActions>
            </button>
          </Item>
        );
      })}
    </ItemGroup>
  );
}
