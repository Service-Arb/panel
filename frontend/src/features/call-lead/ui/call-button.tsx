"use client";

import { Button } from "@evinvest/uikit";
import { Phone } from "lucide-react";

import { type LeadRef, dialable, encodeRef } from "@/entities/lead";
import { useT } from "@/shared/i18n";
import { notifyFailure } from "@/shared/ui/notify";

import { useCallFlow } from "../model/provider";

/**
 * "Call": a `tel:` link that records the attempt as it opens the dialer. When a
 * call to this lead is still waiting for its outcome, the button beside it asks.
 */
export function CallButton({ leadRef, phone }: { leadRef: LeadRef; phone: string }) {
  const t = useT();
  const { state, start, ask } = useCallFlow();
  const pending = state.phase === "dialing" && encodeRef(state.ref) === encodeRef(leadRef);
  const tel = dialable(phone);
  if (!tel) return null;

  return (
    <div className="flex flex-wrap gap-2">
      <Button asChild size="lg">
        <a href={`tel:${tel}`} onClick={() => start(leadRef).catch((e: unknown) => notifyFailure(e, t))}>
          <Phone aria-hidden />
          {t("call.call")}
        </a>
      </Button>
      {pending && (
        <Button variant="outline" size="lg" onClick={ask}>
          {t("call.logOutcome")}
        </Button>
      )}
    </div>
  );
}
