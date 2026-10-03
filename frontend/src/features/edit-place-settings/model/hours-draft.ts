import { DAYS, type Day, type HoursRow, minutesOf } from "@/entities/place";

/** A row as typed: the times are raw text until they parse. `key` keeps React's rows stable. */
export interface HoursDraftRow {
  key: number;
  days: Day[];
  opens: string;
  closes: string;
}

export type RowProblem =
  | { kind: "no_days" }
  | { kind: "bad_opens" }
  | { kind: "bad_closes" }
  | { kind: "same_time" }
  | { kind: "overlap"; days: Day[] };

/** Not a problem: a row that closes at or before it opens runs past midnight, which kitstart accepts. */
export type RowNote = "overnight";

let nextKey = 0;
const key = () => ++nextKey;

export function hoursDraftOf(hours: readonly HoursRow[] | undefined): HoursDraftRow[] {
  return (hours ?? []).map((row) => ({ key: key(), days: [...row.days], opens: row.opens, closes: row.closes }));
}

/** A new row takes the days no row has yet, so "Mon–Fri" then "Sat" is two clicks fewer. */
export function addHoursRow(rows: readonly HoursDraftRow[]): HoursDraftRow[] {
  const taken = new Set(rows.flatMap((r) => r.days));
  return [...rows, { key: key(), days: DAYS.filter((d) => !taken.has(d)), opens: "09:00", closes: "18:00" }];
}

export function updateHoursRow(rows: readonly HoursDraftRow[], rowKey: number, patch: Partial<Omit<HoursDraftRow, "key">>): HoursDraftRow[] {
  return rows.map((r) => (r.key === rowKey ? { ...r, ...patch } : r));
}

export function removeHoursRow(rows: readonly HoursDraftRow[], rowKey: number): HoursDraftRow[] {
  return rows.filter((r) => r.key !== rowKey);
}

export { normaliseTime } from "@/shared/lib/clock";

/** The minutes a row covers on each of its days, past midnight split onto the next. */
function spans(row: HoursDraftRow): { day: Day; from: number; to: number }[] {
  const opens = minutesOf(row.opens);
  const closes = minutesOf(row.closes);
  if (opens === null || closes === null || opens === closes) return [];
  if (opens < closes) return row.days.map((day) => ({ day, from: opens, to: closes }));
  return row.days.flatMap((day) => {
    const next = DAYS[(DAYS.indexOf(day) + 1) % DAYS.length] ?? day;
    return [
      { day, from: opens, to: 24 * 60 },
      { day: next, from: 0, to: closes },
    ];
  });
}

/** What stops a row from saving. Two rows on one day are fine (a lunch break) unless their times overlap. */
export function rowProblems(rows: readonly HoursDraftRow[], row: HoursDraftRow): RowProblem[] {
  const out: RowProblem[] = [];
  if (row.days.length === 0) out.push({ kind: "no_days" });
  const opens = minutesOf(row.opens);
  const closes = minutesOf(row.closes);
  if (opens === null) out.push({ kind: "bad_opens" });
  if (closes === null) out.push({ kind: "bad_closes" });
  if (opens !== null && opens === closes) out.push({ kind: "same_time" });
  const mine = spans(row);
  const clash = new Set<Day>();
  for (const other of rows) {
    if (other.key === row.key) continue;
    for (const a of mine) for (const b of spans(other)) if (a.day === b.day && a.from < b.to && b.from < a.to) clash.add(a.day);
  }
  if (clash.size > 0) out.push({ kind: "overlap", days: DAYS.filter((d) => clash.has(d)) });
  return out;
}

export function rowNotes(row: HoursDraftRow): RowNote[] {
  const opens = minutesOf(row.opens);
  const closes = minutesOf(row.closes);
  return opens !== null && closes !== null && closes < opens ? ["overnight"] : [];
}

export function hoursValid(rows: readonly HoursDraftRow[]): boolean {
  return rows.every((r) => rowProblems(rows, r).length === 0);
}

/** The wire rows, days in week order; no rows → undefined, the site's own hours. */
export function hoursOf(rows: readonly HoursDraftRow[]): HoursRow[] | undefined {
  if (rows.length === 0) return undefined;
  return rows.map((r) => ({ days: DAYS.filter((d) => r.days.includes(d)), opens: r.opens, closes: r.closes }));
}
