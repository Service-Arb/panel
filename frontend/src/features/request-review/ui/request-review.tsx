"use client";

import { type Lead, type LeadEvent } from "@/entities/lead";
import { useLocale, useT } from "@/shared/i18n";
import { formatDateTime } from "@/shared/lib/format";

import { finishedJob } from "../model/gate";
import { AskReview } from "./ask-review";

/**
 * Asking for a Google review, on the lead's card: a line saying when it was
 * asked once it has been; before that, the button for a finished job at a
 * place that has a review link. Once per lead.
 */
export function RequestReview({ lead, events, onAsked }: { lead: Lead; events: readonly LeadEvent[]; onAsked: () => void }) {
  const t = useT();
  const locale = useLocale();
  if (lead.review_requested_at !== null) {
    const at = formatDateTime(lead.review_requested_at, locale);
    const channel = lead.review_requested_channel;
    return <p className="text-sm text-ink-mid">{channel ? t("review.requested", { at, channel: t(`channel.${channel}`) }) : t("review.requestedAt", { at })}</p>;
  }
  // The link is read only for a lead that could be asked.
  if (!finishedJob(lead) || lead.location === null) return null;
  return <AskReview lead={lead} events={events} slug={lead.location} onAsked={onAsked} />;
}
