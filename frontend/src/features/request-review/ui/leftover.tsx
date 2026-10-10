"use client";

import { Button } from "@evinvest/uikit";

import { useT } from "@/shared/i18n";
import { useButtonSize } from "@/shared/ui/touch";

import type { Leftover as Left } from "../model/ask";

/**
 * What the browser would not do for the person: the link to press, the text to
 * select. It stays until closed — the request is recorded, and the card
 * re-reading itself must not take the only copy of the text away.
 */
export function Leftover({ left, onClose }: { left: Left; onClose: () => void }) {
  const t = useT();
  const button = useButtonSize();
  return (
    <div role="status" className="flex flex-col gap-1 text-sm">
      {left.kind === "copy_failed" && (
        <>
          <span className="text-ink-soft">{t("review.copyFailed")}</span>
          <code className="font-mono text-ink select-all whitespace-pre-wrap [overflow-wrap:anywhere]">{left.text}</code>
        </>
      )}
      {left.url !== null && (
        <a href={left.url} target="_blank" rel="noopener noreferrer" className="underline [overflow-wrap:anywhere]">
          {t("review.blocked")}
        </a>
      )}
      <Button type="button" variant="ghost" size={button("sm")} className="self-start" onClick={onClose}>
        {t("review.dismiss")}
      </Button>
    </div>
  );
}
