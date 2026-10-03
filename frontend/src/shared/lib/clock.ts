/** A wall-clock time, `HH:MM`, 00:00–23:59 (kitstart's own pattern). */
const TIME = /^([01]\d|2[0-3]):([0-5]\d)$/;

/** Minutes since midnight of an `HH:MM`, or null when it is not one. */
export function minutesOf(time: string): number | null {
  const m = TIME.exec(time);
  return m ? Number(m[1]) * 60 + Number(m[2]) : null;
}

/** "8:00", "0800", "8h00", "8.00" → "08:00"; anything else stays as typed, for the problem to show. */
export function normaliseTime(raw: string): string {
  const m = /^\s*(\d{1,2})\s*[:h.]?\s*(\d{2})\s*$/i.exec(raw);
  if (!m) return raw.trim();
  const t = `${(m[1] ?? "").padStart(2, "0")}:${m[2] ?? ""}`;
  return minutesOf(t) === null ? raw.trim() : t;
}
