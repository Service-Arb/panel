import { type LeadRef, leadPath } from "@/entities/lead";
import { http, ignoreBody } from "@/shared/api";
import { object, str } from "@/shared/lib/parse";

import type { CallOutcome } from "../model/call-flow";

const attemptParser = object({ attempt_id: str });

/** `call.attempted`: the time is the backend's, stamped when the button was pressed. */
export async function attemptCall(ref: LeadRef): Promise<string> {
  return (await http.send("POST", `${leadPath(ref)}/calls/attempt`, undefined, attemptParser)).attempt_id;
}

export async function logCallOutcome(ref: LeadRef, attemptId: string, outcome: CallOutcome): Promise<void> {
  await http.send("POST", `${leadPath(ref)}/calls/${encodeURIComponent(attemptId)}/outcome`, { outcome }, ignoreBody);
}
