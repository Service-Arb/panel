"use client";

import { useRouter } from "next/navigation";
import { useEffect } from "react";

import { startRouteFor, useMe } from "@/entities/session";

/** `/`: where the caller starts (spec §10). */
export function StartRedirect() {
  const me = useMe();
  const router = useRouter();
  useEffect(() => {
    router.replace(startRouteFor(me));
  }, [me, router]);
  return null;
}
