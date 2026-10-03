import { type BookingConfig, type BookingProblem, checkBookingConfig } from "@/entities/place";
import { type BookingProvider, PAGE_PROVIDERS, type PageProvider } from "@/shared/config/booking";

/** The booking block as typed. No default chosen and no page typed: the site's own booking. */
export interface BookingDraft {
  default: BookingProvider | null;
  urls: Record<PageProvider, string>;
}

/** A problem the form alone can have: pages typed with no default picked. */
export type BookingDraftProblem = BookingProblem | "default_choose";

export function bookingDraftOf(b: BookingConfig | undefined): BookingDraft {
  return {
    default: b?.default ?? null,
    urls: { link: b?.providers.link?.url ?? "", google_calendar: b?.providers.google_calendar?.url ?? "", cal_com: b?.providers.cal_com?.url ?? "" },
  };
}

const typed = (d: BookingDraft) => PAGE_PROVIDERS.filter((p) => d.urls[p].trim() !== "");

/** The setting to save, or null for none (the site's own). Each page trimmed, empty ones left out. */
export function bookingOf(d: BookingDraft): BookingConfig | null {
  const pages = typed(d);
  if (d.default === null && pages.length === 0) return null;
  const providers: BookingConfig["providers"] = {};
  for (const p of pages) providers[p] = { url: d.urls[p].trim() };
  return { default: d.default ?? "manual", providers };
}

/**
 * Problems keyed as the server's 422 keys them, so its answer and the form's
 * own check land on the same field. Checked by the same rules the server keeps.
 */
export function bookingDraftProblems(d: BookingDraft): Map<string, BookingDraftProblem> {
  const config = bookingOf(d);
  if (config === null) return new Map();
  const problems: Map<string, BookingDraftProblem> = checkBookingConfig(config);
  if (d.default === null) problems.set("booking.default", "default_choose");
  return problems;
}

/** The 422 key of a provider's page. */
export const urlKey = (p: PageProvider) => `booking.providers.${p}.url`;
