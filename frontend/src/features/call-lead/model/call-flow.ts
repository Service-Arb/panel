import type { LeadRef } from "@/entities/lead";

/** What a person can know about a call without telephony (spec §10a). */
export const CALL_OUTCOMES = ["answered", "no_answer", "wrong_number", "later"] as const;
export type CallOutcome = (typeof CALL_OUTCOMES)[number];

export interface CallApi {
  attempt(ref: LeadRef): Promise<string>;
  outcome(ref: LeadRef, attemptId: string, outcome: CallOutcome): Promise<void>;
}

/** The page's visibility, as `document.visibilityState` and `visibilitychange` give it. */
export interface Visibility {
  hidden(): boolean;
  subscribe(onChange: () => void): () => void;
}

export type CallState =
  | { phase: "idle" }
  /** The dialer is open; the outcome is asked for when the person comes back to the tab. */
  | { phase: "dialing"; ref: LeadRef }
  | { phase: "asking"; ref: LeadRef };

/**
 * The semi-manual call (spec §10a). `start` records `call.attempted` at once and
 * lets the `tel:` link open the dialer; leaving the tab and coming back to it
 * (`visibilitychange`) asks how the call ended, one tap. Where the dialer never
 * hides the page (a desktop without a phone app), `ask` opens the same question
 * from a button.
 */
export class CallFlow {
  private state: CallState = { phase: "idle" };
  private attempt: Promise<string> | null = null;
  private leftPage = false;
  private readonly unsubscribe: () => void;

  constructor(
    private readonly api: CallApi,
    private readonly visibility: Visibility,
    private readonly onChange: (state: CallState) => void,
  ) {
    this.unsubscribe = visibility.subscribe(() => this.onVisibility());
  }

  get current(): CallState {
    return this.state;
  }

  /** Records the attempt; rejects (and forgets the call) if the backend refused it. */
  async start(ref: LeadRef): Promise<void> {
    this.leftPage = this.visibility.hidden();
    this.attempt = this.api.attempt(ref);
    this.set({ phase: "dialing", ref });
    try {
      await this.attempt;
    } catch (e) {
      this.attempt = null;
      this.set({ phase: "idle" });
      throw e;
    }
  }

  ask(): void {
    if (this.state.phase === "dialing") this.set({ phase: "asking", ref: this.state.ref });
  }

  /** Closing the question without an answer keeps the call open for `ask`. */
  dismiss(): void {
    if (this.state.phase === "asking") {
      this.leftPage = false;
      this.set({ phase: "dialing", ref: this.state.ref });
    }
  }

  async answer(outcome: CallOutcome): Promise<void> {
    if (this.state.phase !== "asking" || !this.attempt) return;
    const { ref } = this.state;
    const attemptId = await this.attempt;
    await this.api.outcome(ref, attemptId, outcome);
    this.attempt = null;
    this.set({ phase: "idle" });
  }

  dispose(): void {
    this.unsubscribe();
  }

  private onVisibility(): void {
    if (this.state.phase !== "dialing") return;
    if (this.visibility.hidden()) this.leftPage = true;
    else if (this.leftPage) this.set({ phase: "asking", ref: this.state.ref });
  }

  private set(state: CallState): void {
    this.state = state;
    this.onChange(state);
  }
}

export function documentVisibility(doc: Document): Visibility {
  return {
    hidden: () => doc.visibilityState === "hidden",
    subscribe: (onChange) => {
      doc.addEventListener("visibilitychange", onChange);
      return () => doc.removeEventListener("visibilitychange", onChange);
    },
  };
}
