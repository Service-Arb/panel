"use client";

import { useSyncExternalStore } from "react";

import { SIGN_IN_PATH } from "@/shared/api";
import { useT } from "@/shared/i18n";
import { RemoteElement } from "@/shared/mfe";

const never = () => () => {};
// the panel names its CSRF cookie by the scheme it is served over: known in the browser only
const csrfCookie = () => (window.location.protocol === "https:" ? "__Host-sa_csrf" : "sa_csrf");

/** review_archive's dashboard, its API and bundle forwarded by the panel (docs/ARCHITECTURE.md, Forward). */
export function ReviewArchiveView() {
  const t = useT();
  const csrf = useSyncExternalStore(never, csrfCookie, () => null);
  if (csrf === null) return null;
  return (
    <RemoteElement
      className="h-full"
      tag="mfe-review-archive-dashboard"
      scriptUrl="/review_archive/mfe/mfe-review-archive-dashboard.js"
      attributes={{ api: "/api/review_archive", "sign-in": SIGN_IN_PATH, "csrf-cookie": csrf, base: "/review_archive" }}
      fallback={<p className="p-6 text-muted-foreground">{t("reviewArchive.loading")}</p>}
    />
  );
}
