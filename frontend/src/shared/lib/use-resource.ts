"use client";

import { useCallback, useEffect, useState } from "react";

import { ApiError, type ApiFailure } from "@/shared/api";

export type Resource<T> =
  | { status: "loading" }
  | { status: "ok"; data: T }
  | { status: "error"; failure: ApiFailure | { kind: "invalid"; message: string } };

function failureOf(e: unknown) {
  if (e instanceof ApiError) return e.failure;
  return { kind: "invalid" as const, message: e instanceof Error ? e.message : String(e) };
}

/**
 * One read, redone whenever `key` changes or `reload` is called. The answer to
 * an earlier key never overwrites a later one.
 */
export function useResource<T>(key: string, load: () => Promise<T>): Resource<T> & { reload: () => void } {
  const [state, setState] = useState<{ key: string; tick: number; value: Resource<T> }>({
    key,
    tick: 0,
    value: { status: "loading" },
  });
  const [tick, setTick] = useState(0);
  const reload = useCallback(() => setTick((n) => n + 1), []);

  useEffect(() => {
    let live = true;
    load().then(
      (data) => live && setState({ key, tick, value: { status: "ok", data } }),
      (e: unknown) => live && setState({ key, tick, value: { status: "error", failure: failureOf(e) } }),
    );
    return () => {
      live = false;
    };
    // `load` is a fresh closure every render; `key` names what it reads.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key, tick]);

  const current: Resource<T> = state.key === key && state.tick === tick ? state.value : { status: "loading" };
  return { ...current, reload };
}
