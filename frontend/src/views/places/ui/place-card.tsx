"use client";

import { Card, CardContent, CardHeader, CardTitle, Progress } from "@evinvest/uikit";

import { type MessageKey, useT } from "@/shared/i18n";
import { formatShare, shareOf } from "@/shared/lib/share";

import type { PlaceRow } from "../model/aggregate";

const STEPS: readonly { key: MessageKey; of: "contacted" | "won" | "paid" }[] = [
  { key: "places.contacted", of: "contacted" },
  { key: "places.won", of: "won" },
  { key: "places.paid", of: "paid" },
];

/**
 * One location's mini funnel against its leads. A bar is a proportion, so it is
 * drawn only where a percent could be said too (spec §10.1); a small sample is
 * "n of m" and nothing more.
 */
export function PlaceCard({ row, minSample }: { row: PlaceRow; minSample: number }) {
  const t = useT();
  return (
    <Card className="gap-3 py-4">
      <CardHeader className="px-4">
        <CardTitle className="wrap-anywhere text-sm">
          {row.brand} · {row.location ?? t("places.unknown")}
        </CardTitle>
      </CardHeader>
      <CardContent className="flex flex-col gap-2 px-4">
        <div className="grid grid-cols-(--grid-place-step) items-center gap-2 text-sm">
          <span className="text-ink-mid">{t("places.leads")}</span>
          <span />
          <span className="tabular-nums text-ink">{row.leads}</span>
        </div>
        {STEPS.map(({ key, of }) => {
          const share = shareOf(row[of], row.leads, minSample);
          return (
            <div key={key} className="grid grid-cols-(--grid-place-step) items-center gap-2 text-sm">
              <span className="text-ink-mid">{t(key)}</span>
              {share.small_sample || share.percent === null ? <span /> : <Progress value={share.percent} aria-label={t(key)} />}
              <span className="tabular-nums text-ink">{formatShare(share, t)}</span>
            </div>
          );
        })}
      </CardContent>
    </Card>
  );
}
