"use client";

import { Badge, Card, CardAction, CardContent, CardDescription, CardHeader, CardTitle } from "@evinvest/uikit";
import type { ReactNode } from "react";

import { useLocale, useT } from "@/shared/i18n";
import { formatDateTime, formatDay } from "@/shared/lib/format";

import type { PricingItem } from "../model/item";

/** What the brand's sites price from now: the panel's model (since when, saved by whom), or their baked one. */
export function PricingStatus({ item, action }: { item: PricingItem; action?: ReactNode }) {
  const t = useT();
  const locale = useLocale();
  const saved = item.updated_at ? t("pricing.status.saved", { at: formatDateTime(item.updated_at, locale), by: item.updated_by ?? "—" }) : null;
  return (
    <Card>
      <CardHeader>
        <CardTitle className="flex flex-wrap items-center gap-2">
          {t("pricing.status.title")}
          {item.model ? <Badge variant="success">{t("pricing.status.live")}</Badge> : <Badge variant="outline">{t("pricing.status.baked")}</Badge>}
        </CardTitle>
        <CardDescription>{item.model ? t("pricing.status.validFrom", { day: formatDay(item.model.validFrom, locale) }) : t("pricing.status.none")}</CardDescription>
        {action && <CardAction>{action}</CardAction>}
      </CardHeader>
      <CardContent className="flex flex-col gap-1 text-sm text-ink-soft">
        {saved && <span>{saved}</span>}
        <span>{t("pricing.status.locales", { locales: item.locales.join(", ") || "—" })}</span>
      </CardContent>
    </Card>
  );
}
