import { type LeadEvent, dialable } from "@/entities/lead";

/**
 * `wa.me/<digits>?text=…` for the customer's number; none without a number to
 * write to (the message is copied instead). Kept local: the marketing helper is
 * not a dependency of the panel.
 */
export function whatsappUrl(phone: string | null, text: string): string | null {
  const tel = dialable(phone);
  return tel === null ? null : `https://wa.me/${tel.replace(/\D/g, "")}?text=${encodeURIComponent(text)}`;
}

const USERNAME = /^[A-Za-z][A-Za-z0-9_]{3,31}$/;

/** "@jean_d" → "jean_d"; a display name or a number is no username. */
export function telegramUsername(handle: string | null): string | null {
  const name = handle?.trim().replace(/^@/, "") ?? "";
  return USERNAME.test(name) ? name : null;
}

export function telegramUrl(username: string): string {
  return `https://t.me/${username}`;
}

/** The customer's Telegram username, from the latest `lead.messaged` that came over Telegram. */
export function telegramHandleOf(events: readonly LeadEvent[]): string | null {
  const said = events.filter((e) => e.type === "lead.messaged" && e.properties.channel === "telegram");
  const handle = said.at(-1)?.pii?.handle;
  return telegramUsername(typeof handle === "string" ? handle : null);
}
