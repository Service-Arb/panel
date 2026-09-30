"use client";

import { Card, CardContent, CardHeader, CardTitle } from "@evinvest/uikit";

import { type MessageKey, useT } from "@/shared/i18n";
import { formatShare, shareOf } from "@/shared/lib/share";

import type { PlaceRow } from "../model/aggregate";

const STEPS: readonly { key: MessageKey; of: keyof Pick<PlaceRow, "leads" | "contacted" | "won" | "paid"> }[] = [
  { key: "places.leads", of: "leads" },
  { key: "places.contacted", of: "contacted" },
  { key: "places.won", of: "won" },
  { key: "places.paid", of: "paid" },
];

/** One location's mini funnel: counts, bars against its leads, and a share where the sample allows. */
export function PlaceCard({ row }: { row: PlaceRow }) {
  const t = useT();
  return (
    <Card className="gap-3 py-4">
      <CardHeader className="px-4">
        <CardTitle className="text-sm">
          {row.brand} · {row.location ?? t("places.unknown")}
        </CardTitle>
      </CardHeader>
      <CardContent className="flex flex-col gap-2 px-4">
        {STEPS.map(({ key, of }) => {
          const n = row[of];
          return (
            <div key={key} className="grid grid-cols-[6rem_1fr_auto] items-center gap-2 text-sm">
              <span className="text-ink-mid">{t(key)}</span>
              <span className="h-1.5 rounded-full bg-muted" aria-hidden>
                <span className="block h-full rounded-full bg-primary-ink" style={{ width: `${row.leads ? (n / row.leads) * 100 : 0}%` }} />
              </span>
              <span className="tabular-nums text-ink">{of === "leads" ? n : formatShare(shareOf(n, row.leads), t)}</span>
            </div>
          );
        })}
      </CardContent>
    </Card>
  );
}
