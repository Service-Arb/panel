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
  /** False when the browser refused the window. */
  open(url: string): boolean;
  copy(text: string): Promise<void>;
}

export type AskOutcome =
  | { kind: "already" }
  | { kind: "opened" }
  | { kind: "copied" }
  | { kind: "copied_and_opened" }
  /** The window was refused: the link is for the person to press. */
  | { kind: "blocked"; url: string }
  /** The clipboard was refused: the text is for the person to select. */
  | { kind: "copy_failed"; text: string };

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
    return { kind: "copy_failed", text: plan.text };
  }
  if (plan.url === null) return { kind: "copied" };
  return deps.open(plan.url) ? { kind: "copied_and_opened" } : { kind: "blocked", url: plan.url };
}
