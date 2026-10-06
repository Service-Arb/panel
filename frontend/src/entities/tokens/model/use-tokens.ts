"use client";

import { useEffect, useState } from "react";

import { type Tokens, fetchTokens } from "../api/review-archive";

/** `GET /me` there provisions the person and writes the day's renewal under a write lock: not on every focus. */
const MIN_GAP_MS = 60_000;

/**
 * The caller's review_archive tokens, read on mount and again when the window
 * regains focus. `null` while unknown and after any failure: the figure is
 * shown only when review_archive answered for this person.
 */
export function useTokens(): Tokens | null {
  const [tokens, setTokens] = useState<Tokens | null>(null);
  useEffect(() => {
    let live = true;
    let last = -Infinity;
    const read = () => {
      if (performance.now() - last < MIN_GAP_MS) return;
      last = performance.now();
      fetchTokens().then(
        (t) => live && setTokens(t),
        () => live && setTokens(null), // forward off, 403, a blip: the stat is not the place to report it
      );
    };
    read();
    window.addEventListener("focus", read);
    return () => {
      live = false;
      window.removeEventListener("focus", read);
    };
  }, []);
  return tokens;
}
