import { toast } from "@evinvest/uikit";

import { ApiError } from "@/shared/api";
import type { T } from "@/shared/i18n";

import { failureText } from "./failure-text";

/** A failed write, told as a toast; a 401 has already sent the browser to sign in. */
export function notifyFailure(e: unknown, t: T): void {
  if (e instanceof ApiError) {
    if (e.failure.kind !== "unauthenticated") toast.error(failureText(e.failure, t));
    return;
  }
  toast.error(t("state.error", { detail: e instanceof Error ? e.message : String(e) }));
}
