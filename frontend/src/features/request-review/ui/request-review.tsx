"use client";

import { useEffect, useRef, useState } from "react";

import { type Lead, type LeadEvent } from "@/entities/lead";
import { useLocale, useT } from "@/shared/i18n";
import { formatDateTime } from "@/shared/lib/format";

import { type Leftover as Left, leftoverFor } from "../model/ask";
import { finishedJob } from "../model/gate";
import { AskReview } from "./ask-review";
import { Leftover } from "./leftover";

/**
 * Asking for a Google review, on the lead's card: a line saying when it was
 * asked once it has been; before that, the button for a finished job at a
 * place that has a review link. Once per lead.
 */
export function RequestReview({ lead, events, onAsked }: { lead: Lead; events: readonly LeadEvent[]; onAsked: () => void }) {
  // Here, not in the button: asking makes the card re-read, and the button then gives way to the line.
  const [left, setLeft] = useState<{ leadId: string; what: Left } | null>(null);
  const shown = leftoverFor(left, lead.lead_id);
  // The button unmounts as the line appears, so the focus it held would fall to the page: it goes to the line.
  const [justAsked, setJustAsked] = useState(false);
  return (
    <>
      <Asked lead={lead} events={events} justAsked={justAsked} onAsked={() => (setJustAsked(true), onAsked())} onLeft={(what) => setLeft({ leadId: lead.lead_id, what })} />
      {shown && <Leftover left={shown} onClose={() => setLeft(null)} />}
    </>
  );
}

function Asked({ lead, events, justAsked, onAsked, onLeft }: { lead: Lead; events: readonly LeadEvent[]; justAsked: boolean; onAsked: () => void; onLeft: (left: Left) => void }) {
  const t = useT();
  const locale = useLocale();
  const line = useRef<HTMLParagraphElement>(null);
  useEffect(() => {
    if (justAsked) line.current?.focus();
  }, [justAsked, lead.review_requested_at]);
  if (lead.review_requested_at !== null) {
    const at = formatDateTime(lead.review_requested_at, locale);
    const channel = lead.review_requested_channel;
    return <p ref={line} tabIndex={-1} className="text-sm text-ink-mid outline-none">{channel ? t("review.requested", { at, channel: t(`channel.${channel}`) }) : t("review.requestedAt", { at })}</p>;
  }
  // The link is read only for a lead that could be asked.
  if (!finishedJob(lead) || lead.location === null) return null;
  return <AskReview lead={lead} events={events} slug={lead.location} onAsked={onAsked} onLeft={onLeft} />;
}
