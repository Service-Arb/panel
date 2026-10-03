"use client";

import { useCallback, useEffect, useRef, useState } from "react";

import { ApiError, type ApiFailure } from "@/shared/api";

import { type LiveMatch, useLiveSignal } from "./live";

export type Resource<T> =
  | { status: "loading" }
  | { status: "ok"; data: T }
  | { status: "error"; failure: ApiFailure | { kind: "invalid"; message: string } };

function failureOf(e: unknown) {
  if (e instanceof ApiError) return e.failure;
  return { kind: "invalid" as const, message: e instanceof Error ? e.message : String(e) };
}

export interface ResourceOptions {
  /** The live topics this read follows. Whatever it is, a resync (reconnect, poll) re-reads it. */
  live?: LiveMatch;
}

const NO_TOPICS: LiveMatch = [];

/**
 * One read, redone whenever `key` changes or `reload` is called. The answer to
 * an earlier key never overwrites a later one.
 *
 * Stale-while-revalidate: a reload, or a new key in the same `group` (the same
 * record at a newer version), keeps showing the last answer until the next one
 * lands, so a refresh after an action does not blank the screen.
 *
 * A live re-read (a change on a followed topic, a resync) is quiet: if it
 * fails, the last answer stays on screen — the person did not ask for it, and
 * a blip in the background must not replace figures with an error.
 */
export function useResource<T>(key: string, load: () => Promise<T>, group: string = key, options: ResourceOptions = {}): Resource<T> & { reload: () => void } {
  const [state, setState] = useState<{ key: string; group: string; tick: number; value: Resource<T> }>({
    key,
    group,
    tick: 0,
    value: { status: "loading" },
  });
  const [tick, setTick] = useState(0);
  const quiet = useRef(false);
  const reload = useCallback(() => setTick((n) => n + 1), []);
  const refresh = useCallback(() => {
    quiet.current = true;
    setTick((n) => n + 1);
  }, []);
  useLiveSignal(options.live ?? NO_TOPICS, refresh);

  useEffect(() => {
    let live = true;
    const background = quiet.current;
    quiet.current = false;
    load().then(
      (data) => live && setState({ key, group, tick, value: { status: "ok", data } }),
      (e: unknown) =>
        live &&
        setState((prev) =>
          background && prev.group === group && prev.value.status === "ok"
            ? { ...prev, key, tick }
            : { key, group, tick, value: { status: "error", failure: failureOf(e) } },
        ),
    );
    return () => {
      live = false;
    };
    // `load` is a fresh closure every render; `key` names what it reads.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key, tick]);

  const fresh = state.key === key && state.tick === tick;
  const stale = state.group === group && state.value.status === "ok";
  const current: Resource<T> = fresh || stale ? state.value : { status: "loading" };
  return { ...current, reload };
}
