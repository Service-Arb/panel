"use client";

import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuTrigger, toast } from "@evinvest/uikit";
import { useId } from "react";

import { type Lead, type LeadEvent, requestReview, refOf } from "@/entities/lead";
import { may, useMe } from "@/entities/session";
import { useT } from "@/shared/i18n";
import { useKeyedWrite } from "@/shared/lib/use-keyed-write";
import { notifyFailure } from "@/shared/ui/notify";
import { useButtonSize } from "@/shared/ui/touch";

import { type AskPlan, type Leftover, askForReview } from "../model/ask";
import { reviewGate } from "../model/gate";
import { reviewPlans } from "../model/plans";
import { brandLabel } from "../model/message";
import { useReviewPlace } from "../model/use-review-place";
import { ReviewTrigger } from "./review-trigger";

/** `noopener` in the features string makes `open` return null always, so it is cut by hand: null is then a refused window. */
function openWindow(url: string): Window | null {
  const w = window.open(url, "_blank");
  if (w) w.opener = null;
  return w;
}

/** The button (a menu when the customer can be reached two ways), or why it is off. */
export function AskReview({ lead, events, slug, onAsked, onLeft }: { lead: Lead; events: readonly LeadEvent[]; slug: string; onAsked: () => void; onLeft: (left: Leftover) => void }) {
  const t = useT();
  const button = useButtonSize();
  const reasonId = useId();
  const me = useMe();
  const { reviewUrl, brandName } = useReviewPlace(lead.brand, slug);
  const { busy, run } = useKeyedWrite();
  const gate = reviewGate(lead, reviewUrl, may(me, "sa:work:pii:see"));
  if (gate.kind === "hidden" || gate.kind === "asked" || reviewUrl === null) return null;

  const ask = async (plan: AskPlan) => {
    const done = await run(plan.channel, async () => {
      const outcome = await askForReview(plan, { send: (channel) => requestReview(refOf(lead), channel), open: openWindow, copy: (text) => navigator.clipboard.writeText(text) });
      if (outcome.kind === "already") toast.info(t("review.already"));
      else if (outcome.kind === "blocked" || outcome.kind === "copy_failed") onLeft(outcome);
      else toast.positive(t(outcome.kind === "opened" ? "review.saved" : "review.copied"));
      onAsked();
    });
    if (!done.ok) notifyFailure(done.error, t);
  };

  const plans = gate.kind === "ready" ? reviewPlans(lead, events, reviewUrl, brandLabel(brandName, lead.brand)) : [];
  const label = (plan: AskPlan) => t(plan.channel === "telegram" ? "review.via.telegram" : plan.url ? "review.via.whatsapp" : "review.via.whatsappCopy");
  const off = busy || gate.kind !== "ready";
  const trigger = (
    <ReviewTrigger label={t("review.ask")} off={off} reasonId={gate.kind === "needs_pii" ? reasonId : undefined} size={button("lg")} onPress={plans.length === 1 ? () => void ask(plans[0]!) : undefined} />
  );

  return (
    <div className="flex flex-col items-start gap-2">
      {plans.length > 1 ? (
        <DropdownMenu>
          <DropdownMenuTrigger asChild>{trigger}</DropdownMenuTrigger>
          <DropdownMenuContent align="start">
            {plans.map((plan) => (
              <DropdownMenuItem key={plan.channel} onSelect={() => void ask(plan)}>
                {label(plan)}
              </DropdownMenuItem>
            ))}
          </DropdownMenuContent>
        </DropdownMenu>
      ) : (
        trigger
      )}
      {gate.kind === "needs_pii" && (
        <p id={reasonId} className="text-sm text-ink-soft">
          {t("review.needsPii")}
        </p>
      )}
    </div>
  );
}
