import type { ApiFailure } from "@/shared/api";
import type { T } from "@/shared/i18n";

/** What to tell a person about a failed call; the backend's own words where it gave some. */
export function failureText(failure: ApiFailure | { kind: "invalid"; message: string }, t: T): string {
  switch (failure.kind) {
    case "csrf":
      return t("state.csrf");
    case "forbidden":
      return t("state.forbidden");
    case "unavailable":
      return t("state.unavailable.body");
    case "no_access":
      return t("state.noAccess.title");
    case "bad_request":
    case "conflict":
      return failure.message;
    case "invalid_fields":
      return Object.values(failure.fields).join(" · ") || t("state.error", { detail: "422" });
    case "failed":
      return t("state.error", { detail: `HTTP ${failure.status}` });
    case "invalid":
      return t("state.error", { detail: failure.message });
    default:
      return t("state.error", { detail: failure.kind });
  }
}
