import { minutesOf } from "./clock";

const DAY = /^(\d{4})-(\d{2})-(\d{2})$/;

/** A local calendar day as the kit's Calendar hands it, `YYYY-MM-DD`. */
export function dayOfDate(date: Date): string {
  return `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, "0")}-${String(date.getDate()).padStart(2, "0")}`;
}

/** `YYYY-MM-DD` as a local Date at midnight (the Calendar's own unit), or undefined when it is not a day. */
export function dateOfDay(day: string): Date | undefined {
  const m = DAY.exec(day);
  if (!m) return undefined;
  const date = new Date(Number(m[1]), Number(m[2]) - 1, Number(m[3]));
  return dayOfDate(date) === day ? date : undefined;
}

/**
 * The moment a day and an `HH:MM` name in the browser's zone, or null. A time
 * the clocks skip (the spring change) lands on the moment after, as Date does.
 */
export function localInstant(day: string, time: string): Date | null {
  const date = dateOfDay(day);
  const minutes = minutesOf(time);
  if (!date || minutes === null) return null;
  date.setHours(Math.floor(minutes / 60), minutes % 60, 0, 0);
  return date;
}

const pad = (n: number) => String(n).padStart(2, "0");

/**
 * RFC 3339 with the zone's offset, to the second: `2026-10-06T10:00:00+02:00`.
 * `offsetMinutes` is east of UTC (Paris in summer: 120); by default the
 * browser's at that moment, so a slot after the autumn change carries +01:00.
 */
export function rfc3339(at: Date, offsetMinutes: number = -at.getTimezoneOffset()): string {
  const local = new Date(at.getTime() + offsetMinutes * 60_000);
  const sign = offsetMinutes < 0 ? "-" : "+";
  const abs = Math.abs(offsetMinutes);
  const date = `${local.getUTCFullYear()}-${pad(local.getUTCMonth() + 1)}-${pad(local.getUTCDate())}`;
  const time = `${pad(local.getUTCHours())}:${pad(local.getUTCMinutes())}:${pad(local.getUTCSeconds())}`;
  return `${date}T${time}${sign}${pad(Math.floor(abs / 60))}:${pad(abs % 60)}`;
}

/**
 * A slot as a reader takes it in, in their zone and naming it: "Tue 6 Oct,
 * 10:00 – 11:00 GMT+2". `timeZone` is for tests; screens use the browser's.
 */
export function formatSlot(startIso: string, endIso: string | null, locale: string, timeZone?: string): string {
  const start = new Date(startIso);
  if (Number.isNaN(start.getTime())) return startIso;
  const format = new Intl.DateTimeFormat(locale, {
    weekday: "short",
    day: "numeric",
    month: "short",
    hour: "2-digit",
    minute: "2-digit",
    timeZoneName: "short",
    ...(timeZone ? { timeZone } : {}),
  });
  const end = endIso === null ? null : new Date(endIso);
  return end && !Number.isNaN(end.getTime()) && end > start ? format.formatRange(start, end) : format.format(start);
}
