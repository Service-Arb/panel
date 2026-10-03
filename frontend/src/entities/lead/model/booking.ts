import { BOOKING_PROVIDERS } from "@/shared/config/booking";
import { type Infer, type Parser, isoDay, nullable, object, oneOf, str } from "@/shared/lib/parse";

/** `panel_core::booking::BookingStatus`, in its order. */
export const BOOKING_STATUSES = ["none", "requested", "booked", "canceled", "done", "no_show"] as const;
export type BookingStatus = (typeof BOOKING_STATUSES)[number];

/** How a provider's booking found its lead: the site's reference, the contact (a likely match), or an operator's hand. */
export const BOOKING_MATCHES = ["ref", "contact", "manual"] as const;
export type BookingMatch = (typeof BOOKING_MATCHES)[number];

/** The part of the day a visitor asked for (`booking.requested@1`, manual only). */
export const DAY_PARTS = ["morning", "afternoon", "evening"] as const;
export type DayPart = (typeof DAY_PARTS)[number];

const bookingObject = object({
  status: oneOf(BOOKING_STATUSES),
  provider: nullable(oneOf(BOOKING_PROVIDERS)),
  start_at: nullable(str),
  end_at: nullable(str),
  external_ref: nullable(str),
  match: nullable(oneOf(BOOKING_MATCHES)),
  preferred_date: nullable(isoDay),
  preferred_part: nullable(oneOf(DAY_PARTS)),
});
export type LeadBooking = Infer<typeof bookingObject>;

export const NO_BOOKING: LeadBooking = {
  status: "none",
  provider: null,
  start_at: null,
  end_at: null,
  external_ref: null,
  match: null,
  preferred_date: null,
  preferred_part: null,
};

/** `Lead.booking`: always sent; a lead read from a backend before bookings has none. */
export const leadBookingParser: Parser<LeadBooking> = (v, path) => (v === undefined ? NO_BOOKING : bookingObject(v, path));

/** What an operator may do from where a booking stands (`BookingStatus::allows`): anything else answers 409. */
export type BookingAction = "set" | "clear" | "done" | "no_show" | "canceled";

export function bookingActions(status: BookingStatus): BookingAction[] {
  const actions: BookingAction[] = [];
  if (status !== "done") actions.push("set");
  if (status === "booked") actions.push("done", "no_show", "canceled");
  if (status === "requested" || status === "booked" || status === "canceled" || status === "no_show") actions.push("clear");
  return actions;
}
