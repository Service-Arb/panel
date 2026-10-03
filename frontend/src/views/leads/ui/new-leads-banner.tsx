"use client";

import { Button } from "@evinvest/uikit";

import { useT } from "@/shared/i18n";
import { useButtonSize } from "@/shared/ui/touch";

/**
 * Arrivals wait here instead of pushing the rows down under the pointer. The
 * live region is always mounted, so a screen reader hears the count appear.
 */
export function NewLeadsBanner({ count, suspect, onShow }: { count: number; suspect: number; onShow: () => void }) {
  const t = useT();
  const button = useButtonSize();
  return (
    <div aria-live="polite" className="empty:hidden">
      {count > 0 && (
        <div data-enter="rise" className="flex items-center justify-between gap-3 rounded-lg border border-primary-ink/40 bg-primary-ink/10 px-3 py-2">
          <span className="text-sm font-medium text-ink">
            {t("leads.live.new", { n: count })}
            {/* Counted with the rest, as the nav counts them, but said: they may not be worth a call. */}
            {suspect > 0 && <span className="font-normal text-accent-warn"> {t("leads.live.suspect", { n: suspect })}</span>}
          </span>
          <Button variant="outline" size={button("sm")} onClick={onShow}>
            {t("leads.live.show")}
          </Button>
        </div>
      )}
    </div>
  );
}
