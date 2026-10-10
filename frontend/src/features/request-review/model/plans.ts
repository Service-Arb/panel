import { type Lead, type LeadEvent, contactOf } from "@/entities/lead";

import type { AskPlan } from "./ask";
import { telegramHandleOf, telegramUrl, whatsappUrl } from "./links";
import { reviewMessage } from "./message";

/**
 * The ways to ask this customer: WhatsApp (the chat when we have a number,
 * else the text is copied) and, when they wrote to us on Telegram, Telegram.
 * A Telegram-only customer with no number is not offered a WhatsApp with
 * nowhere to go.
 */
export function reviewPlans(lead: Lead, events: readonly LeadEvent[], reviewUrl: string, brand: string): AskPlan[] {
  const contact = contactOf(lead.pii);
  const text = reviewMessage({ locale: lead.locale, name: contact.name, brand, link: reviewUrl });
  const username = telegramHandleOf(events);
  const plans: AskPlan[] = [];
  const wa = whatsappUrl(contact.phone, text);
  if (wa !== null || username === null) plans.push({ channel: "whatsapp", text, url: wa });
  if (username !== null) plans.push({ channel: "telegram", text, url: telegramUrl(username) });
  return plans;
}
