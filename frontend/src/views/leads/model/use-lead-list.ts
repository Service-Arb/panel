"use client";

import { useCallback, useEffect, useState } from "react";

import { type Lead, type LeadFilter, fetchLeads } from "@/entities/lead";
import { ApiError, type ApiFailure } from "@/shared/api";

type ListState =
  | { status: "loading" }
  | { status: "ok"; leads: Lead[]; cursor: string | null; more: boolean }
  | { status: "error"; failure: ApiFailure | { kind: "invalid"; message: string } };

function failureOf(e: unknown) {
  return e instanceof ApiError ? e.failure : { kind: "invalid" as const, message: e instanceof Error ? e.message : String(e) };
}

/** The queue under a filter, newest first, a page at a time; a new filter or `reload` starts over. */
export function useLeadList(filter: LeadFilter) {
  const key = JSON.stringify(filter);
  const [tick, setTick] = useState(0);
  const [state, setState] = useState<{ key: string; tick: number; list: ListState }>({ key, tick, list: { status: "loading" } });

  useEffect(() => {
    let live = true;
    const f: LeadFilter = JSON.parse(key);
    fetchLeads(f, null).then(
      (page) => live && setState({ key, tick, list: { status: "ok", leads: page.leads, cursor: page.next_cursor, more: false } }),
      (e: unknown) => live && setState({ key, tick, list: { status: "error", failure: failureOf(e) } }),
    );
    return () => {
      live = false;
    };
  }, [key, tick]);

  const list: ListState = state.key === key && state.tick === tick ? state.list : { status: "loading" };

  const loadMore = async () => {
    if (list.status !== "ok" || !list.cursor || list.more) return;
    setState({ key, tick, list: { ...list, more: true } });
    try {
      const page = await fetchLeads(filter, list.cursor);
      setState({ key, tick, list: { status: "ok", leads: [...list.leads, ...page.leads], cursor: page.next_cursor, more: false } });
    } catch (e) {
      setState({ key, tick, list: { status: "error", failure: failureOf(e) } });
    }
  };

  const reload = useCallback(() => setTick((n) => n + 1), []);
  return { list, loadMore, reload };
}
