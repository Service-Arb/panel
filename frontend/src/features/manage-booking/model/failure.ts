import { ApiError } from "@/shared/api";
import type { MessageKey } from "@/shared/i18n";

/**
 * What a refused booking write means to the operator. A 409 is the booking
 * moved on meanwhile (an operator here, the provider there): the card is read
 * again and says how it stands. Null: the general failure text applies.
 */
export function bookingRefusal(e: unknown): { key: MessageKey; detail?: string; reread: boolean } | null {
  if (!(e instanceof ApiError)) return null;
  switch (e.failure.kind) {
    case "conflict":
      return { key: "booking.conflict", reread: true };
    case "not_found":
      return { key: "booking.notFound", reread: true };
    case "bad_request":
      return { key: "booking.badTime", detail: e.failure.message, reread: false };
    default:
      return null;
  }
}
