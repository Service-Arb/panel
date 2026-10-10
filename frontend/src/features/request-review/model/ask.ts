import type { Messenger, ReviewRequested } from "@/entities/lead";

/** What to do once the server has the request: the text, and where it goes. */
export interface AskPlan {
  channel: Messenger;
  text: string;
  /** WhatsApp: the chat with the number (none without one: copy). Telegram: the customer's chat. */
  url: string | null;
}

export interface AskDeps {
  send(channel: Messenger): Promise<ReviewRequested>;
  /** The window it opened, null when the browser refused it. */
  open(url: string): Window | null;
  copy(text: string): Promise<void>;
}

export type AskOutcome =
  | { kind: "already" }
  | { kind: "opened" }
  | { kind: "copied" }
  | { kind: "copied_and_opened" }
  /** The window was refused: the link is for the person to press. */
  | { kind: "blocked"; url: string }
  /** The clipboard was refused: the text is for the person to select, and the chat (when there is one) to press. */
  | { kind: "copy_failed"; text: string; url: string | null };

/** What is left for the person to do by hand. */
export type Leftover = Extract<AskOutcome, { kind: "blocked" | "copy_failed" }>;

/**
 * The server first: the request is recorded before anything opens, as a call
 * is before the dialer. A refusal (it throws) opens and copies nothing; an
 * earlier ask (`already_requested`) does not open the chat a second time.
 */
export async function askForReview(plan: AskPlan, deps: AskDeps): Promise<AskOutcome> {
  const { already_requested } = await deps.send(plan.channel);
  if (already_requested) return { kind: "already" };
  if (plan.channel === "whatsapp" && plan.url !== null) return deps.open(plan.url) ? { kind: "opened" } : { kind: "blocked", url: plan.url };
  try {
    await deps.copy(plan.text);
  } catch {
    // No clipboard outside a secure context, or the page lost the gesture.
    return { kind: "copy_failed", text: plan.text, url: plan.url };
  }
  if (plan.url === null) return { kind: "copied" };
  return deps.open(plan.url) ? { kind: "copied_and_opened" } : { kind: "blocked", url: plan.url };
}

/** What is held for the card on screen: another lead's leftover is not shown on this one. */
export function leftoverFor(held: { leadId: string; what: Leftover } | null, leadId: string): Leftover | null {
  return held?.leadId === leadId ? held.what : null;
}
