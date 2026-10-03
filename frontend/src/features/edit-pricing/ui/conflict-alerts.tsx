"use client";

import { Alert, AlertDescription, AlertTitle, Button } from "@evinvest/uikit";

import type { PricingItem } from "@/entities/pricing";
import { useLocale, useT } from "@/shared/i18n";
import { formatDateTime } from "@/shared/lib/format";
import { useButtonSize } from "@/shared/ui/touch";

const whoWhen = (item: PricingItem, locale: string) => ({ at: item.updated_at ? formatDateTime(item.updated_at, locale) : "—", by: item.updated_by ?? "—" });

/**
 * A save refused with 409: someone saved first. The draft stays; the person
 * either drops it for the fresh pricing or writes it over the fresh pricing.
 */
export function ConflictAlert({ current, onTakeFresh, onOverwrite }: { current: PricingItem | null; onTakeFresh: () => void; onOverwrite: () => void }) {
  const t = useT();
  const locale = useLocale();
  const button = useButtonSize();
  return (
    <Alert variant="destructive" className="flex flex-col gap-2" role="alert">
      <AlertTitle>{t("pricing.conflict.title")}</AlertTitle>
      <AlertDescription>{current ? t("pricing.conflict.body", whoWhen(current, locale)) : t("pricing.conflict.unknown")}</AlertDescription>
      <div className="flex flex-wrap gap-2">
        <Button type="button" variant="outline" size={button("sm")} onClick={onTakeFresh}>
          {t("pricing.conflict.load")}
        </Button>
        {current && (
          <Button type="button" variant="destructive" size={button("sm")} onClick={onOverwrite}>
            {t("pricing.conflict.overwrite")}
          </Button>
        )}
      </div>
    </Alert>
  );
}

/** Someone saved while this draft holds edits: nothing is lost yet, and a save would be refused. */
export function FresherAlert({ fresher, onTakeFresh }: { fresher: PricingItem; onTakeFresh: () => void }) {
  const t = useT();
  const locale = useLocale();
  const button = useButtonSize();
  return (
    <Alert className="flex flex-col gap-2 border-accent-warn" data-enter="rise">
      <AlertTitle>{t("pricing.fresher.title")}</AlertTitle>
      <AlertDescription>{t("pricing.fresher.body", whoWhen(fresher, locale))}</AlertDescription>
      <Button type="button" variant="outline" size={button("sm")} className="self-start" onClick={onTakeFresh}>
        {t("pricing.conflict.load")}
      </Button>
    </Alert>
  );
}
