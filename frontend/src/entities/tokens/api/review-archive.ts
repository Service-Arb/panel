import { createHttp } from "@/shared/api";
import { type Infer, type Parser, arrayOf, isoDay, num, object } from "@/shared/lib/parse";

/** The forward to review_archive (`/api/review_archive/*`): its 401 is not the panel's session ending. */
const forward = createHttp({
  fetch: (input, init) => fetch(input, init),
  cookie: () => (typeof document === "undefined" ? "" : document.cookie),
  // The panel's own reads send a lapsed session to sign-in; a refusal here only hides what it would show.
  onUnauthenticated: () => {},
});

const tokensParser = object({ balance: num, daily: num, cap: num });
export type Tokens = Infer<typeof tokensParser>;

const meParser: Parser<{ tokens: Tokens }> = object({ tokens: tokensParser });

export async function fetchTokens(): Promise<Tokens> {
  return (await forward.get("/api/review_archive/me", meParser)).tokens;
}

const usageParser = object({
  days: arrayOf(object({ day: isoDay, walks: num, tokens: num })),
  places_tracked: num,
});
export type Usage = Infer<typeof usageParser>;

export function fetchUsage(): Promise<Usage> {
  return forward.get("/api/review_archive/me/usage", usageParser);
}

