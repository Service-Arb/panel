"use client";

import { useRouter } from "next/navigation";
import { useEffect } from "react";

import { startRouteFor, useMe } from "@/entities/session";

/** `/`: an operator starts on new leads, an admin on the overview (spec §10). */
export function StartRedirect() {
  const { role } = useMe();
  const router = useRouter();
  useEffect(() => {
    router.replace(startRouteFor(role));
  }, [role, router]);
  return null;
}
