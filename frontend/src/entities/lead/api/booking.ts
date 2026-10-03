import { IDEMPOTENCY_HEADER, http, ignoreBody } from "@/shared/api";

import { type LeadRef, leadPath } from "../model/lead";

/** `POST …/booking`'s bodies; the instants are RFC 3339 with their offset. */
export type SlotBody = { action: "set"; start_at: string; end_at?: string } | { action: "clear" };

/** `POST …/booking/status`: closing a booked slot. */
export type CloseBody = { status: "done" | "no_show" | "canceled" };

/** 201 (200 for a retry under the same key); 400 a bad instant, 404 no lead, 409 not from where it stands. */
export async function writeSlot(ref: LeadRef, body: SlotBody, idempotencyKey: string): Promise<void> {
  await http.send("POST", `${leadPath(ref)}/booking`, body, ignoreBody, { [IDEMPOTENCY_HEADER]: idempotencyKey });
}

/** 201; 409 unless the booking stands at `booked`. */
export async function closeBooking(ref: LeadRef, body: CloseBody, idempotencyKey: string): Promise<void> {
  await http.send("POST", `${leadPath(ref)}/booking/status`, body, ignoreBody, { [IDEMPOTENCY_HEADER]: idempotencyKey });
}
