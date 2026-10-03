"use client";

import { Separator, Settled, Skeleton } from "@evinvest/uikit";
import { useState } from "react";

import { type LeadCard, type LeadRef, contactOf, encodeRef, fetchLeadCard } from "@/entities/lead";
import { CallButton } from "@/features/call-lead";
import { StageActions, hasMoves } from "@/features/move-stage";
import { PaymentForm, takesPayment } from "@/features/record-payment";
import { useT } from "@/shared/i18n";
import type { ChangedEvent } from "@/shared/lib/live";
import { useResource } from "@/shared/lib/use-resource";
import { EmptyState } from "@/shared/ui/empty-state";
import { ErrorState } from "@/shared/ui/error-state";

import { type Known, nextKnown, updatedElsewhere } from "../model/card-updates";
import { EventList } from "./event-list";
import { LeadDetails } from "./lead-details";
import { UpdatedNote } from "./updated-note";

const follows = (ref: LeadRef) => (e: ChangedEvent) => e.topic === "lead" && (e.brand_id === null || e.brand_id === ref.brand) && (e.id === null || e.id === ref.lead);

/**
 * The lead card. It follows the lead live; a change the person did not make
 * here (`version` counts theirs) is said in a note at the top.
 */
export function LeadCardPanel({ leadRef, version, onChanged }: { leadRef: LeadRef; version: number; onChanged: () => void }) {
  const id = encodeRef(leadRef);
  const card = useResource(`card:${id}:${version}`, () => fetchLeadCard(leadRef), `card:${id}`, { live: follows(leadRef) });
  const [known, setKnown] = useState<Known>({ version, at: null, awaiting: null });
  const at = card.status === "ok" ? card.data.lead.last_event_at : null;
  const next = nextKnown(known, version, at);
  if (next !== null) setKnown(next);

  return (
    <Settled loading={card.status === "loading"} skeleton={<Skeleton className="h-64 w-full" />}>
      {card.status === "error" && <ErrorState failure={card.failure} onRetry={card.reload} />}
      {card.status === "ok" && <CardBody card={card.data} leadRef={leadRef} elsewhere={updatedElsewhere(known, version, at)} onChanged={onChanged} />}
    </Settled>
  );
}

/** Who, then every action a next step needs, then what happened so far. */
function CardBody({ card, leadRef, elsewhere, onChanged }: { card: LeadCard; leadRef: LeadRef; elsewhere: boolean; onChanged: () => void }) {
  const t = useT();
  const { lead, events } = card;
  const phone = contactOf(lead.pii).phone;
  return (
    <div className="flex flex-col gap-4">
      {elsewhere && <UpdatedNote key={lead.last_event_at} at={lead.last_event_at} type={events.at(-1)?.type ?? null} />}
      <LeadDetails lead={lead} />
      {phone && <CallButton leadRef={leadRef} phone={phone} />}
      {hasMoves(lead.stage) && (
        <section className="flex flex-col gap-2">
          <h3 className="text-xs font-medium uppercase tracking-wide text-ink-soft">{t("card.actions")}</h3>
          <StageActions lead={lead} onMoved={onChanged} />
        </section>
      )}
      {takesPayment(lead.stage) && (
        <section className="flex flex-col gap-2">
          <h3 className="text-xs font-medium uppercase tracking-wide text-ink-soft">{t("payment.title")}</h3>
          <PaymentForm lead={lead} onSaved={onChanged} />
        </section>
      )}
      <Separator />
      <section className="flex flex-col gap-2">
        <h3 className="text-xs font-medium uppercase tracking-wide text-ink-soft">{t("card.events")}</h3>
        {events.length > 0 ? <EventList events={events} /> : <EmptyState className="p-4" title={t("card.events.empty")} />}
      </section>
    </div>
  );
}
