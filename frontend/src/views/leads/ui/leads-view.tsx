"use client";

import { Button, Skeleton } from "@evinvest/uikit";
import { usePathname, useRouter, useSearchParams } from "next/navigation";
import { useCallback, useState } from "react";

import { type Lead, type LeadFilter, type LeadRef, decodeRef, encodeRef, refOf } from "@/entities/lead";
import { brandsOf, locationsOf, usePlaces } from "@/entities/place";
import { CallFlowProvider, OutcomeSheet } from "@/features/call-lead";
import { CreateLeadButton } from "@/features/create-lead";
import { LeadFilters, leadFilterFrom, paramsWith } from "@/features/lead-filters";
import { useT } from "@/shared/i18n";
import { DESKTOP_QUERY, useMediaQuery } from "@/shared/lib/use-media-query";
import { EmptyState } from "@/shared/ui/empty-state";
import { ErrorState } from "@/shared/ui/error-state";
import { notifyFailure } from "@/shared/ui/notify";
import { PageHeader } from "@/shared/ui/page-header";
import { PanelOverlay } from "@/shared/ui/panel-overlay";
import { useButtonSize } from "@/shared/ui/touch";

import { useLeadList } from "../model/use-lead-list";
import { LeadCardPanel } from "./lead-card-panel";
import { LeadList } from "./lead-list";
import { LeadTable } from "./lead-table";

export function LeadsView() {
  const t = useT();
  const button = useButtonSize();
  const params = useSearchParams();
  const router = useRouter();
  const pathname = usePathname();
  const isDesktop = useMediaQuery(DESKTOP_QUERY);
  const filter = leadFilterFrom(params);
  const open = decodeRef(params.get("lead"));
  const { list, loadMore, reload } = useLeadList(filter);
  const [version, setVersion] = useState(0);

  const go = (next: URLSearchParams) => router.replace(`${pathname}?${next.toString()}`, { scroll: false });
  const setFilter = (patch: Partial<LeadFilter>) => go(paramsWith(params, patch));
  const openLead = (ref: LeadRef | null) => {
    const next = new URLSearchParams(params);
    if (ref) next.set("lead", encodeRef(ref));
    else next.delete("lead");
    go(next);
  };
  const changed = useCallback(() => {
    setVersion((v) => v + 1);
    reload();
  }, [reload]);

  const created = (ref: LeadRef) => {
    changed();
    openLead(ref);
  };
  const filtered = filter.stage !== null || filter.brand !== null || filter.location !== null || filter.overdue;

  const leads: Lead[] = list.status === "ok" ? list.leads : [];
  const places = usePlaces(version);
  const brands = brandsOf(places, filter.brand);
  const locations = locationsOf(places, filter.brand, filter.location);

  return (
    <CallFlowProvider onLogged={changed}>
      <div className="flex flex-col gap-4 p-4 md:p-6">
        <PageHeader title={t("leads.title")}>
          <CreateLeadButton brands={brands} places={places} onCreated={created} />
        </PageHeader>
        <LeadFilters filter={filter} brands={brands} locations={locations} onChange={setFilter} />
        {list.status === "loading" && <Skeleton className="h-64 w-full" />}
        {list.status === "error" && <ErrorState failure={list.failure} onRetry={reload} />}
        {list.status === "ok" && leads.length === 0 && (
          <EmptyState title={t("leads.empty")} description={t(filtered ? "leads.empty.filtered" : "leads.empty.none")}>
            {filtered && (
              <Button variant="outline" size={button()} onClick={() => go(new URLSearchParams())}>
                {t("leads.resetFilters")}
              </Button>
            )}
            <CreateLeadButton brands={brands} places={places} onCreated={created} />
          </EmptyState>
        )}
        {leads.length > 0 && (isDesktop ? <LeadTable leads={leads} onOpen={(l) => openLead(refOf(l))} /> : <LeadList leads={leads} onOpen={(l) => openLead(refOf(l))} />)}
        {list.status === "ok" && list.cursor && (
          <Button variant="outline" size={button()} className="self-center" disabled={list.more} onClick={() => loadMore().catch((e: unknown) => notifyFailure(e, t))}>
            {t("leads.more")}
          </Button>
        )}
      </div>
      <PanelOverlay open={open !== null} onOpenChange={(o) => !o && openLead(null)} title={t("card.title")} desktop="sheet">
        {open && <LeadCardPanel leadRef={open} version={version} onChanged={changed} />}
      </PanelOverlay>
      <OutcomeSheet />
    </CallFlowProvider>
  );
}
