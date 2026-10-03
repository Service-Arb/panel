import type { T } from "@/shared/i18n";

/** A wait as the coarsest unit that is not zero: "19 min", "3 h", "2 d". */
export function formatWait(seconds: number, t: T): string {
  const mins = Math.max(0, Math.floor(seconds / 60));
  if (mins < 60) return t("time.min", { n: mins });
  const hours = Math.floor(mins / 60);
  if (hours < 48) return t("time.hour", { n: hours });
  return t("time.day", { n: Math.floor(hours / 24) });
}

/** Money is kept in minor units and shown to the whole unit (spec §10.1). */
export function formatMoney(minor: number, currency: string, locale: string): string {
  try {
    return new Intl.NumberFormat(locale, { style: "currency", currency, maximumFractionDigits: 0 }).format(minor / 100);
  } catch {
    return `${Math.round(minor / 100)} ${currency}`;
  }
}

const DECIMAL = /^-?\d+\.\d{2}$/;
const isDecimal = (s: string): s is Intl.StringNumericLiteral => DECIMAL.test(s);

/**
 * Integer cents as Intl's exact decimal string: the number is never divided as
 * a float, so 1 234,50 € is what was stored to the cent.
 */
function decimalOf(cents: number): string {
  const abs = Math.abs(cents);
  return `${cents < 0 ? "-" : ""}${Math.floor(abs / 100)}.${String(abs % 100).padStart(2, "0")}`;
}

/**
 * A price to the cent, as quoted: the cents shown only when there are any
 * ("89 €", "89,50 €"). For figures read at a glance `formatMoney` rounds instead.
 */
export function formatCents(cents: number, currency: string, locale: string): string {
  const decimal = decimalOf(cents);
  const digits = cents % 100 === 0 ? 0 : 2;
  try {
    if (!isDecimal(decimal)) throw new RangeError(`not a decimal: ${decimal}`);
    return new Intl.NumberFormat(locale, { style: "currency", currency, minimumFractionDigits: digits, maximumFractionDigits: digits }).format(decimal);
  } catch {
    return `${decimal} ${currency}`;
  }
}

/** "95", "95.5", "95,50" → 9550; anything else, or more than two decimals, → null. */
export function parseMoney(raw: string): number | null {
  const s = raw.trim().replace(",", ".");
  if (!/^\d+(\.\d{1,2})?$/.test(s)) return null;
  const [whole = "0", frac = ""] = s.split(".");
  return Number(whole) * 100 + Number(frac.padEnd(2, "0"));
}

export function formatDateTime(iso: string, locale: string): string {
  const d = new Date(iso);
  return Number.isNaN(d.getTime()) ? iso : d.toLocaleString(locale, { dateStyle: "short", timeStyle: "short" });
}

/**
 * A write's moment to the second: two saves a minute apart — or the same
 * minute — must read as two moments.
 */
export function formatMoment(iso: string, locale: string): string {
  const d = new Date(iso);
  return Number.isNaN(d.getTime()) ? iso : d.toLocaleString(locale, { dateStyle: "short", timeStyle: "medium" });
}

/** A calendar day `YYYY-MM-DD` as the reader writes dates; read as UTC so no zone moves it a day. */
export function formatDay(day: string, locale: string): string {
  const d = new Date(`${day}T00:00:00Z`);
  return Number.isNaN(d.getTime()) ? day : d.toLocaleDateString(locale, { dateStyle: "medium", timeZone: "UTC" });
}

/** A UTC calendar day, `YYYY-MM-DD`, as the funnel's `from`/`to` take it. */
export function utcDay(d: Date): string {
  return d.toISOString().slice(0, 10);
}

export function daysAgo(now: Date, days: number): Date {
  return new Date(now.getTime() - days * 86_400_000);
}

/**
 * A configured share, 0–100, to a tenth at most ("50 %", "33,3 %"). Not for a
 * measured rate: those go through `formatShare`, which withholds small samples.
 */
export function formatPercent(percent: number, locale: string): string {
  return new Intl.NumberFormat(locale, { style: "percent", maximumFractionDigits: 1 }).format(percent / 100);
}
