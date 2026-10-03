"use client";

import { useEffect } from "react";

/** `(3) Leads — Service-Arb panel` while `count` is above zero; the title as it was otherwise. */
export function useTitleCount(count: number, label: string): void {
  useEffect(() => {
    if (count <= 0) return;
    const base = document.title;
    document.title = `(${count}) ${label} — ${base}`;
    return () => {
      document.title = base;
    };
  }, [count, label]);
}
