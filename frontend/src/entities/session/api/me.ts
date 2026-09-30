import { http } from "@/shared/api";
import { object, oneOf, str } from "@/shared/lib/parse";

import { type Me, ROLES } from "../model/role";

const meParser = object({ user_id: str, role: oneOf(ROLES), email: str, preferred_name: str });

export function fetchMe(): Promise<Me> {
  return http.get("/api/v1/me", meParser);
}

export async function signOut(): Promise<void> {
  await http.send("POST", "/auth/logout", undefined, () => null);
}
