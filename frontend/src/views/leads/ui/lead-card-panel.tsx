"use client";

import { Separator, Skeleton } from "@evinvest/uikit";

import { type LeadRef, contactOf, encodeRef, fetchLeadCard } from "@/entities/lead";
import { CallButton } from "@/features/call-lead";
import { StageActions } from "@/features/move-stage";
import { PaymentForm, takesPayment } from "@/features/record-payment";
import { useT } from "@/shared/i18n";
import { useResource } from "@/shared/lib/use-resource";
import { ErrorState } from "@/shared/ui/error-state";

import { EventList } from "./event-list";
import { LeadDetails } from "./lead-details";

/** The lead card: who, then every action a next step needs, then what happened so far. */
export function LeadCardPanel({ leadRef, version, onChanged }: { leadRef: LeadRef; version: number; onChanged: () => void }) {
  const t = useT();
  const card = useResource(`card:${encodeRef(leadRef)}:${version}`, () => fetchLeadCard(leadRef));

  if (card.status === "loading") return <Skeleton className="h-64 w-full" />;
  if (card.status === "error") return <ErrorState failure={card.failure} onRetry={card.reload} />;

  const { lead, events } = card.data;
  const phone = contactOf(lead.pii).phone;

  return (
    <div className="flex flex-col gap-4">
      <LeadDetails lead={lead} />
      {phone && <CallButton leadRef={leadRef} phone={phone} />}
      <section className="flex flex-col gap-2">
        <h3 className="text-xs font-medium uppercase tracking-wide text-ink-soft">{t("card.actions")}</h3>
        <StageActions lead={lead} onMoved={onChanged} />
      </section>
      {takesPayment(lead.stage) && (
        <section className="flex flex-col gap-2">
          <h3 className="text-xs font-medium uppercase tracking-wide text-ink-soft">{t("payment.title")}</h3>
          <PaymentForm lead={lead} onSaved={onChanged} />
        </section>
      )}
      <Separator />
      <section className="flex flex-col gap-2">
        <h3 className="text-xs font-medium uppercase tracking-wide text-ink-soft">{t("card.events")}</h3>
        <EventList events={events} />
      </section>
    </div>
  );
}
