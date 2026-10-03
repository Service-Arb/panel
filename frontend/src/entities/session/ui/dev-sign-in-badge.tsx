"use client";

import { Badge, cn } from "@evinvest/uikit";

import { useT } from "@/shared/i18n";

import { useMe } from "../model/context";

/** Loud on purpose: a backend that signs anyone in must never pass for the real one. */
export function DevSignInBadge({ className }: { className?: string }) {
  const t = useT();
  const { dev_sign_in } = useMe();
  if (!dev_sign_in) return null;
  return (
    <Badge variant="outline" title={t("session.devSignIn.hint")} className={cn("border-accent-warn bg-accent-warn/15 text-accent-warn uppercase tracking-wide", className)}>
      {t("session.devSignIn")}
    </Badge>
  );
}
