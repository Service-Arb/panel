"use client";

import { useCallback } from "react";

import { signOut } from "@/entities/session";
import { ROUTES } from "@/shared/config/routes";
import { useT } from "@/shared/i18n";
import { notifyFailure } from "@/shared/ui/notify";

export function useSignOut(): () => void {
  const t = useT();
  return useCallback(() => {
    signOut().then(
      () => window.location.assign(ROUTES.signedOut),
      (e: unknown) => notifyFailure(e, t),
    );
  }, [t]);
}
