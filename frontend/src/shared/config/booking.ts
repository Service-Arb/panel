/**
 * The booking providers, a closed set (`panel_core::booking::Provider`, the
 * kitstart fixtures' README). `manual` has no page: the site promises a call
 * and an operator records the slot, so it is always available and never
 * configured; the others are the ones a place configures with a URL.
 */
export { BOOKING_PROVIDERS, PAGE_PROVIDERS } from "./generated";
export type { BookingProvider, PageProvider } from "./generated";
