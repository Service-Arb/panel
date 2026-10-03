"use client";

import { Button } from "@evinvest/uikit";
import { usePathname, useRouter, useSearchParams } from "next/navigation";
import { useCallback, useState } from "react";

import { type LeadFilter, type LeadRef, decodeRef, encodeRef, fetchLeadCounts } from "@/entities/lead";
import { brandsOf, locationsOf, usePlaces } from "@/entities/place";
import { CallFlowProvider, OutcomeSheet } from "@/features/call-lead";
import { CreateLeadButton } from "@/features/create-lead";
import { LeadFilters, leadFilterFrom, narrows, paramsWith } from "@/features/lead-filters";
import { useT } from "@/shared/i18n";
import { useResource } from "@/shared/lib/use-resource";
import { EmptyState } from "@/shared/ui/empty-state";
import { PanelOverlay } from "@/shared/ui/panel-overlay";
import { ScreenFrame } from "@/shared/ui/screen-frame";
import { useButtonSize } from "@/shared/ui/touch";

import { LeadCardPanel } from "./lead-card-panel";
import { LeadQueue } from "./lead-queue";

export function LeadsView() {
  const t = useT();
  const button = useButtonSize();
  const params = useSearchParams();
  const router = useRouter();
  const pathname = usePathname();
  const filter = leadFilterFrom(params);
  const open = decodeRef(params.get("lead"));
  const [version, setVersion] = useState(0);

  const go = (next: URLSearchParams) => router.replace(`${pathname}?${next.toString()}`, { scroll: false });
  const setFilter = (patch: Partial<LeadFilter>) => go(paramsWith(params, patch));
  const openLead = (ref: LeadRef | null) => {
    const next = new URLSearchParams(params);
    if (ref) next.set("lead", encodeRef(ref));
    else next.delete("lead");
    go(next);
  };
  const changed = useCallback(() => setVersion((v) => v + 1), []);
  const created = (ref: LeadRef) => {
    changed();
    openLead(ref);
  };
  const filtered = narrows(filter);
  const where = `${filter.brand ?? ""}/${filter.location ?? ""}`;
  const counts = useResource(`counts:${where}:${version}`, () => fetchLeadCounts(filter), `counts:${where}`, { live: ["leads", "lead"] });

  const places = usePlaces(version);
  const brands = brandsOf(places, filter.brand);
  const locations = locationsOf(places, filter.brand, filter.location);
  const create = <CreateLeadButton brands={brands} places={places} onCreated={created} />;

  return (
    <CallFlowProvider onLogged={changed}>
      <ScreenFrame title={t("leads.title")} actions={create}>
        <LeadFilters filter={filter} brands={brands} locations={locations} counts={counts.status === "ok" ? counts.data : null} onChange={setFilter} />
        <LeadQueue
          filter={filter}
          version={version}
          onOpen={openLead}
          empty={
            <EmptyState title={t("leads.empty")} description={t(filtered ? "leads.empty.filtered" : "leads.empty.none")}>
              {filtered && (
                <Button variant="outline" size={button()} onClick={() => go(new URLSearchParams())}>
                  {t("leads.resetFilters")}
                </Button>
              )}
              {create}
            </EmptyState>
          }
        />
      </ScreenFrame>
      <PanelOverlay open={open !== null} onOpenChange={(o) => !o && openLead(null)} title={t("card.title")} desktop="sheet">
        {open && <LeadCardPanel leadRef={open} version={version} onChanged={changed} />}
      </PanelOverlay>
      <OutcomeSheet />
    </CallFlowProvider>
  );
}
