import { type Infer, arrayOf, isoDay, nullable, num, object, str } from "@/shared/lib/parse";
import { shareParser } from "@/shared/lib/share";

const weekParser = object({
  /** The Monday (UTC) of the week the jobs were completed. */
  week: isoDay,
  brand: str,
  /** Null for the leads that name no place. */
  location: nullable(str),
  /** `n` of the `of` jobs finished that week were asked for a review. */
  share: shareParser,
});
export type ReviewWeek = Infer<typeof weekParser>;

/** `GET /review-requests`: how many of the finished jobs were asked for a Google review, per place and week. */
export const reviewRequestsParser = object({
  from: str,
  to: str,
  brand: nullable(str),
  min_sample: num,
  total: shareParser,
  weeks: arrayOf(weekParser),
});
export type ReviewRequests = Infer<typeof reviewRequestsParser>;

const DAY_MS = 86_400_000;

/**
 * Whether the window cuts the week: it starts after the Monday, or ends before
 * the Sunday. A cut week counts fewer jobs than a whole one and reads as such.
 */
export function isPartialWeek(week: string, from: string, to: string): boolean {
  const monday = Date.parse(`${week}T00:00:00Z`);
  if (Number.isNaN(monday)) return false;
  const sunday = new Date(monday + 6 * DAY_MS).toISOString().slice(0, 10);
  return from > week || to < sunday;
}

/** The weeks newest first, each with its places in a stable order. */
export function groupByWeek(weeks: readonly ReviewWeek[]): { week: string; rows: ReviewWeek[] }[] {
  const sorted = [...weeks].sort((a, b) => b.week.localeCompare(a.week) || a.brand.localeCompare(b.brand) || (a.location ?? "").localeCompare(b.location ?? ""));
  const groups: { week: string; rows: ReviewWeek[] }[] = [];
  for (const row of sorted) {
    const last = groups.at(-1);
    if (last?.week === row.week) last.rows.push(row);
    else groups.push({ week: row.week, rows: [row] });
  }
  return groups;
}
