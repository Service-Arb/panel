import { http } from "@/shared/api";
import { type Parser, arrayOf, bool, object, oneOf, str } from "@/shared/lib/parse";

import { type Caller, PERMISSIONS } from "../model/generated";

/** A backend from before the flag says nothing: that is not dev sign-in. */
const flag: Parser<boolean> = (v, path) => (v === undefined ? false : bool(v, path));

const meParser = object({ user_id: str, email: str, permissions: arrayOf(oneOf(PERMISSIONS)), preferred_name: str, dev_sign_in: flag });

export function fetchMe(): Promise<Caller> {
  return http.get("/api/v1/me", meParser);
}

export async function signOut(): Promise<void> {
  await http.send("POST", "/auth/logout", undefined, () => null);
}
