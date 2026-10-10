"use client";

import { Badge, Card, CardContent, CardHeader, CardTitle } from "@evinvest/uikit";

import { type ReviewRequests, groupByWeek, isPartialWeek } from "@/entities/review-request";
import { useLocale, useT } from "@/shared/i18n";
import { formatDay } from "@/shared/lib/format";
import { type Share } from "@/shared/lib/share";
import { EmptyState } from "@/shared/ui/empty-state";

/** "3 of 5", and "(60%)" where the sample allows a percent; never a percent the backend withheld. */
function ShareText({ share }: { share: Share }) {
  const t = useT();
  const showPercent = share.percent !== null && !share.small_sample;
  return (
    <span className="inline-flex flex-wrap items-center justify-end gap-x-1.5 tabular-nums">
      <span className="font-medium text-ink">{t("share.nOf", { n: share.n, of: share.of })}</span>
      {showPercent && <span className="text-ink-soft">({t("share.percent", { percent: share.percent ?? 0 })})</span>}
      {share.small_sample && <Badge variant="outline">{t("reviews.smallSample")}</Badge>}
    </span>
  );
}

/**
 * Of the jobs finished, how many were asked for a Google review: the total,
 * then each week's places, newest first. A week the period cuts says so — it
 * counts fewer jobs than a whole one.
 */
export function ReviewRequestsBlock({ reviews }: { reviews: ReviewRequests }) {
  const t = useT();
  const locale = useLocale();

  return (
    <Card className="gap-3 py-4">
      <CardHeader className="px-4">
        <CardTitle className="text-xs font-medium uppercase tracking-wide text-ink-soft">{t("reviews.title")}</CardTitle>
        <p className="text-sm text-ink-soft">{t("reviews.caption")}</p>
      </CardHeader>
      <CardContent className="flex flex-col gap-3 px-4">
        {reviews.total.of === 0 ? (
          <EmptyState className="p-6" title={t("reviews.empty")} description={t("funnel.empty.body")} />
        ) : (
          <>
            <p className="flex items-center justify-between gap-3 text-sm">
              <span className="text-ink-mid">{t("reviews.total")}</span>
              <ShareText share={reviews.total} />
            </p>
            {groupByWeek(reviews.weeks).map(({ week, rows }) => (
              <section key={week} className="flex flex-col gap-1 border-t border-border pt-2">
                <h3 className="flex flex-wrap items-center gap-2 text-xs font-medium text-ink-soft">
                  {t("reviews.week", { day: formatDay(week, locale) })}
                  {isPartialWeek(week, reviews.from, reviews.to) && <Badge variant="outline">{t("reviews.partialWeek")}</Badge>}
                </h3>
                <ul className="flex flex-col gap-1">
                  {rows.map((w) => (
                    <li key={`${w.brand}/${w.location ?? ""}`} className="flex items-center justify-between gap-3 text-sm">
                      <span className="min-w-0 truncate text-ink-mid">
                        {w.brand} · {w.location ?? t("places.unknown")}
                      </span>
                      <ShareText share={w.share} />
                    </li>
                  ))}
                </ul>
              </section>
            ))}
          </>
        )}
      </CardContent>
    </Card>
  );
}
