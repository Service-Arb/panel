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

/** A UTC calendar day, `YYYY-MM-DD`, as the funnel's `from`/`to` take it. */
export function utcDay(d: Date): string {
  return d.toISOString().slice(0, 10);
}

export function daysAgo(now: Date, days: number): Date {
  return new Date(now.getTime() - days * 86_400_000);
}
