"use client";

import { Button, toast } from "@evinvest/uikit";
import { useState } from "react";

import { type Lead, type Messenger, markMessaged, refOf } from "@/entities/lead";
import { useT } from "@/shared/i18n";
import { notifyFailure } from "@/shared/ui/notify";
import { useButtonSize } from "@/shared/ui/touch";

/**
 * The operator saw the customer's message (matched by its ref) in WhatsApp
 * Business or the bot's chat: `lead.messaged` from the panel. A repeat is a
 * no-op on the server, so a double tap or a retry does no harm.
 */
export function MarkMessagedButton({ lead, channel, onMarked }: { lead: Lead; channel: Messenger; onMarked: () => void }) {
  const t = useT();
  const button = useButtonSize();
  const [busy, setBusy] = useState(false);

  const mark = async () => {
    setBusy(true);
    try {
      await markMessaged(refOf(lead), channel);
      toast.positive(t("move.saved"));
      onMarked();
    } catch (e) {
      notifyFailure(e, t);
    } finally {
      setBusy(false);
    }
  };

  return (
    <Button size={button()} variant="outline" className="self-start" disabled={busy} onClick={() => void mark()}>
      {t("messenger.mark")}
    </Button>
  );
}
