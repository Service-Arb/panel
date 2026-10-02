import type { T } from "@/shared/i18n";

import { DAYS, type Day, type HoursRow } from "../model/settings";

const TIME = /^([01]\d|2[0-3]):([0-5]\d)$/;

/** Minutes since midnight of an `HH:MM`, or null when it is not one (kitstart's own pattern). */
export function minutesOf(time: string): number | null {
  const m = TIME.exec(time);
  return m ? Number(m[1]) * 60 + Number(m[2]) : null;
}

export interface LocalTime {
  day: Day;
  minutes: number;
}

/** The weekday and minute of `at` on the place's wall clock. */
export function localTimeOf(at: Date, timeZone: string): LocalTime {
  const parts = new Intl.DateTimeFormat("en-US", { timeZone, weekday: "long", hour: "2-digit", minute: "2-digit", hourCycle: "h23" }).formatToParts(at);
  const part = (type: string) => parts.find((p) => p.type === type)?.value ?? "";
  const day = DAYS.find((d) => d === part("weekday")) ?? "Monday";
  return { day, minutes: Number(part("hour")) * 60 + Number(part("minute")) };
}

const previousDay = (day: Day): Day => DAYS[(DAYS.indexOf(day) + DAYS.length - 1) % DAYS.length] ?? day;

/**
 * Whether the hours say "open" at a local time. A row that closes at or before
 * it opens runs past midnight into the next day.
 */
export function isOpenAt(hours: readonly HoursRow[], at: LocalTime): boolean {
  return hours.some((row) => {
    const opens = minutesOf(row.opens);
    const closes = minutesOf(row.closes);
    if (opens === null || closes === null) return false;
    if (opens < closes) return row.days.includes(at.day) && at.minutes >= opens && at.minutes < closes;
    return (row.days.includes(at.day) && at.minutes >= opens) || (row.days.includes(previousDay(at.day)) && at.minutes < closes);
  });
}

/** Days in week order, runs of three or more joined: "Mon–Fri, Sun". */
export function formatDays(days: readonly Day[], t: T): string {
  const idx = [...new Set(days.map((d) => DAYS.indexOf(d)))].sort((a, b) => a - b);
  const runs: number[][] = [];
  for (const i of idx) {
    const run = runs.at(-1);
    if (run && run.at(-1) === i - 1) run.push(i);
    else runs.push([i]);
  }
  const name = (i: number) => t(`day.short.${DAYS[i] ?? "Monday"}`);
  return runs
    .flatMap((run) => (run.length >= 3 ? [`${name(run[0] ?? 0)}–${name(run.at(-1) ?? 0)}`] : run.map(name)))
    .join(", ");
}

/** "Mon–Fri 08:00–19:00; Sat 09:00–12:00". */
export function formatHours(hours: readonly HoursRow[], t: T): string {
  return hours.map((row) => `${formatDays(row.days, t)} ${row.opens}–${row.closes}`).join("; ");
}
