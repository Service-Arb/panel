"use client";

import { Button, Settled, Skeleton } from "@evinvest/uikit";
import type { ReactNode } from "react";

import { type LeadFilter, type LeadRef, refOf } from "@/entities/lead";
import { useT } from "@/shared/i18n";
import { DESKTOP_QUERY, useMediaQuery } from "@/shared/lib/use-media-query";
import { ErrorState } from "@/shared/ui/error-state";
import { notifyFailure } from "@/shared/ui/notify";
import { useButtonSize } from "@/shared/ui/touch";

import { useLeadList } from "../model/use-lead-list";
import { LeadList } from "./lead-list";
import { LeadTable } from "./lead-table";
import { NewLeadsBanner } from "./new-leads-banner";

export interface LeadQueueProps {
  filter: LeadFilter;
  /** Bumped by the person's own writes: the queue re-reads at once. */
  version: number;
  onOpen: (ref: LeadRef) => void;
  /** Shown when the filter matches nothing. */
  empty: ReactNode;
}

/** The queue itself: a table on desktop, a list on a phone, arrivals held in a banner above it. */
export function LeadQueue({ filter, version, onOpen, empty }: LeadQueueProps) {
  const t = useT();
  const button = useButtonSize();
  const isDesktop = useMediaQuery(DESKTOP_QUERY);
  const { list, loadMore, reload, pending, showPending, flash } = useLeadList(filter, version);

  const leads = list.status === "ok" ? list.leads : [];
  const open = (lead: (typeof leads)[number]) => onOpen(refOf(lead));
  return (
    <Settled loading={list.status === "loading"} skeleton={<Skeleton className="h-64 w-full" />} className="flex flex-col gap-4">
      <NewLeadsBanner count={pending.length} suspect={pending.filter((l) => l.suspect !== null).length} onShow={showPending} />
      {list.status === "error" && <ErrorState failure={list.failure} onRetry={reload} />}
      {list.status === "ok" && leads.length === 0 && empty}
      {leads.length > 0 && (isDesktop ? <LeadTable leads={leads} flash={flash} onOpen={open} /> : <LeadList leads={leads} flash={flash} onOpen={open} />)}
      {list.status === "ok" && list.cursor && (
        <Button variant="outline" size={button()} className="self-center" disabled={list.more} onClick={() => loadMore().catch((e: unknown) => notifyFailure(e, t))}>
          {t("leads.more")}
        </Button>
      )}
    </Settled>
  );
}
