"use client";

import { Button, Empty, EmptyHeader, EmptyTitle, Skeleton } from "@evinvest/uikit";
import { usePathname, useRouter, useSearchParams } from "next/navigation";
import { useCallback, useState } from "react";

import { type Lead, type LeadFilter, type LeadRef, decodeRef, encodeRef, refOf } from "@/entities/lead";
import { CallFlowProvider, OutcomeSheet } from "@/features/call-lead";
import { CreateLeadButton } from "@/features/create-lead";
import { LeadFilters, leadFilterFrom, paramsWith } from "@/features/lead-filters";
import { KNOWN_BRANDS } from "@/shared/config/brands";
import { useT } from "@/shared/i18n";
import { DESKTOP_QUERY, useMediaQuery } from "@/shared/lib/use-media-query";
import { ErrorState } from "@/shared/ui/error-state";
import { PageHeader } from "@/shared/ui/page-header";
import { PanelOverlay } from "@/shared/ui/panel-overlay";

import { useLeadList } from "../model/use-lead-list";
import { LeadCardPanel } from "./lead-card-panel";
import { LeadList } from "./lead-list";
import { LeadTable } from "./lead-table";

const uniq = (xs: string[]) => [...new Set(xs)].sort();

export function LeadsView() {
  const t = useT();
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

  const leads: Lead[] = list.status === "ok" ? list.leads : [];
  const places = leads.flatMap((l) => (l.location ? [{ brand: l.brand, location: l.location }] : []));
  const brands = uniq([...KNOWN_BRANDS, ...leads.map((l) => l.brand)]);
  const locations = uniq(places.filter((p) => !filter.brand || p.brand === filter.brand).map((p) => p.location).concat(filter.location ?? []));

  return (
    <CallFlowProvider onLogged={changed}>
      <div className="flex flex-col gap-4 p-4 md:p-6">
        <PageHeader title={t("leads.title")}>
          <CreateLeadButton brands={brands} places={places} onCreated={(ref) => {
              changed();
              openLead(ref);
            }} />
        </PageHeader>
        <LeadFilters filter={filter} brands={brands} locations={locations} onChange={setFilter} />
        {list.status === "loading" && <Skeleton className="h-64 w-full" />}
        {list.status === "error" && <ErrorState failure={list.failure} onRetry={reload} />}
        {list.status === "ok" && leads.length === 0 && (
          <Empty>
            <EmptyHeader>
              <EmptyTitle>{t("leads.empty")}</EmptyTitle>
            </EmptyHeader>
          </Empty>
        )}
        {leads.length > 0 && (isDesktop ? <LeadTable leads={leads} onOpen={(l) => openLead(refOf(l))} /> : <LeadList leads={leads} onOpen={(l) => openLead(refOf(l))} />)}
        {list.status === "ok" && list.cursor && (
          <Button variant="outline" className="self-center" disabled={list.more} onClick={() => void loadMore()}>
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
