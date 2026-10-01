"use client";

import { Card, CardContent, CardHeader, CardTitle, Progress } from "@evinvest/uikit";

import { useT } from "@/shared/i18n";
import { formatShare } from "@/shared/lib/share";

import type { PlaceRow } from "../model/rows";

/**
 * One location's mini funnel against its leads. A bar is a proportion, so it is
 * drawn only where a percent could be said too (spec §10.1); a small sample is
 * "n of m" and nothing more.
 */
export function PlaceCard({ row }: { row: PlaceRow }) {
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
        {row.steps.map(({ stage, share }) => (
          <div key={stage} className="grid grid-cols-(--grid-place-step) items-center gap-2 text-sm">
            <span className="text-ink-mid">{t(`places.${stage}`)}</span>
            {share.small_sample || share.percent === null ? <span /> : <Progress value={share.percent} aria-label={t(`places.${stage}`)} />}
            <span className="tabular-nums text-ink">{formatShare(share, t)}</span>
          </div>
        ))}
      </CardContent>
    </Card>
  );
}
