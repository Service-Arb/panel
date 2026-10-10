"use client";

import { Button, DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuTrigger, toast } from "@evinvest/uikit";
import { MessageSquareHeart } from "lucide-react";
import { useId, useState } from "react";

import { type Lead, type LeadEvent, requestReview, refOf } from "@/entities/lead";
import { may, useMe } from "@/entities/session";
import { useT } from "@/shared/i18n";
import { useKeyedWrite } from "@/shared/lib/use-keyed-write";
import { notifyFailure } from "@/shared/ui/notify";
import { useButtonSize } from "@/shared/ui/touch";

import { type AskOutcome, type AskPlan, askForReview } from "../model/ask";
import { reviewGate } from "../model/gate";
import { reviewPlans } from "../model/plans";
import { brandLabel } from "../model/message";
import { useReviewPlace } from "../model/use-review-place";

const openWindow = (url: string) => window.open(url, "_blank", "noopener,noreferrer") !== null;

/** The button (a menu when the customer can be reached two ways), or why it is off. */
export function AskReview({ lead, events, slug, onAsked }: { lead: Lead; events: readonly LeadEvent[]; slug: string; onAsked: () => void }) {
  const t = useT();
  const button = useButtonSize();
  const reasonId = useId();
  const me = useMe();
  const { reviewUrl, brandName } = useReviewPlace(lead.brand, slug);
  const { busy, run } = useKeyedWrite();
  const [left, setLeft] = useState<Extract<AskOutcome, { kind: "blocked" | "copy_failed" }> | null>(null);
  const gate = reviewGate(lead, reviewUrl, may(me, "sa:work:pii:see"));
  if (gate.kind === "hidden" || gate.kind === "asked" || reviewUrl === null) return null;

  const ask = async (plan: AskPlan) => {
    const done = await run(plan.channel, async () => {
      const outcome = await askForReview(plan, { send: (channel) => requestReview(refOf(lead), channel), open: openWindow, copy: (text) => navigator.clipboard.writeText(text) });
      if (outcome.kind === "already") toast.info(t("review.already"));
      else if (outcome.kind === "blocked" || outcome.kind === "copy_failed") setLeft(outcome);
      else toast.positive(t(outcome.kind === "opened" ? "review.saved" : "review.copied"));
      onAsked();
    });
    if (!done.ok) notifyFailure(done.error, t);
  };

  const plans = gate.kind === "ready" ? reviewPlans(lead, events, reviewUrl, brandLabel(brandName, lead.brand)) : [];
  const label = (plan: AskPlan) => t(plan.channel === "telegram" ? "review.via.telegram" : plan.url ? "review.via.whatsapp" : "review.via.whatsappCopy");
  const trigger = (
    <Button type="button" variant="outline" size={button("lg")} className="self-start" disabled={busy || gate.kind !== "ready"} aria-describedby={gate.kind === "ready" ? undefined : reasonId} onClick={plans.length === 1 ? () => void ask(plans[0]!) : undefined}>
      <MessageSquareHeart aria-hidden />
      {t("review.ask")}
    </Button>
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
      {left && <Left outcome={left} />}
    </div>
  );
}

/** What the browser would not do for the person: the link to press, or the text to select. */
function Left({ outcome }: { outcome: Extract<AskOutcome, { kind: "blocked" | "copy_failed" }> }) {
  const t = useT();
  if (outcome.kind === "blocked") {
    return (
      <a href={outcome.url} target="_blank" rel="noopener noreferrer" className="text-sm underline">
        {t("review.blocked")}
      </a>
    );
  }
  return (
    <div className="flex flex-col gap-1 text-sm">
      <span className="text-ink-soft">{t("review.copyFailed")}</span>
      <code className="font-mono text-ink select-all whitespace-pre-wrap">{outcome.text}</code>
    </div>
  );
}
