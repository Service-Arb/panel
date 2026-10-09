import type { AccessRequest } from "@/entities/session";
import { http } from "@/shared/api";
import { arrayOf, object, oneOf, str } from "@/shared/lib/parse";

import { NEEDS, type Need } from "../model/access";

const requestParser = object({ need: oneOf(NEEDS), requested_at: str });

export function fetchMyRequests(): Promise<AccessRequest[]> {
  return http.get("/api/v1/access/requests/mine", object({ requests: arrayOf(requestParser) })).then((r) => r.requests);
}

/** 201 made now, 200 the one standing; 409 when the need is held already. */
export function requestAccess(need: Need): Promise<AccessRequest> {
  return http.send("POST", "/api/v1/access/requests", { need }, requestParser);
}
