/**
 * The booking providers, a closed set (`panel_core::booking::Provider`, the
 * kitstart fixtures' README). `manual` has no page: the site promises a call
 * and an operator records the slot, so it is always available and never
 * configured.
 */
export const BOOKING_PROVIDERS = ["manual", "link", "google_calendar", "cal_com"] as const;
export type BookingProvider = (typeof BOOKING_PROVIDERS)[number];

/** The providers with a page of their own: the ones a place configures with a URL. */
export const PAGE_PROVIDERS = ["link", "google_calendar", "cal_com"] as const;
export type PageProvider = (typeof PAGE_PROVIDERS)[number];
