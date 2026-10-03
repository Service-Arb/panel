"use client";

import { useEffect, useState } from "react";

import { fetchLeadCounts } from "@/entities/lead";
import { type ChangedEvent, useLiveSignal } from "@/shared/lib/live";
import { useWindowFocus } from "@/shared/lib/use-window-focus";

import { NOTHING_UNSEEN, type Unseen, newLeads, noteChanges, parseSeen, sectionOf, seenKey, visit } from "./unseen";

export interface Marks {
  /** New leads since the leads screen was last in front of the person. */
  leads: number;
  /** New leads since this tab lost focus: the tab title's count. */
  away: number;
  places: number;
  experiments: boolean;
  sources: boolean;
  /** Bookings without a lead came since the leads screen was last open. */
  bookings: boolean;
}

function readSeen(userId: string): number | null {
  try {
    return parseSeen(window.localStorage.getItem(seenKey(userId)));
  } catch {
    // Storage refused (a private window): the count starts from this visit.
    return null;
  }
}

function writeSeen(userId: string, seen: number): void {
  try {
    window.localStorage.setItem(seenKey(userId), String(seen));
  } catch {
    // As above: kept for this page's life only.
  }
}

/**
 * The nav's marks. Leads are counted from the total (it only grows), the
 * baseline kept per person across reloads; places, experiments and sources
 * from the live events since their screen was last open, for this page's life.
 */
export function useMarks(userId: string, pathname: string): Marks {
  const at = sectionOf(pathname);
  const focused = useWindowFocus();
  const [unseen, setUnseen] = useState<Unseen>(NOTHING_UNSEEN);
  const [total, setTotal] = useState<number | null>(null);
  const [seen, setSeen] = useState<number | null>(() => readSeen(userId));
  const [awayFrom, setAwayFrom] = useState<number | null>(null);

  // Adjusted during render rather than in effects: each is a fact about this
  // render (the screen open, the window focused), and an effect would paint the
  // stale mark once first.
  const visited = visit(unseen, at);
  if (visited !== unseen) setUnseen(visited);
  if (total !== null && (seen === null || seen > total || (at === "leads" && focused && seen !== total))) setSeen(total);
  if (total !== null && !focused && awayFrom === null) setAwayFrom(total);
  if (focused && awayFrom !== null) setAwayFrom(null);

  useEffect(() => {
    if (seen !== null) writeSeen(userId, seen);
  }, [userId, seen]);

  // Re-counted on every lead change; an answer overtaken by a newer count is dropped.
  const [recount, setRecount] = useState(0);
  useEffect(() => {
    let live = true;
    fetchLeadCounts({ brand: null, location: null }).then(
      (counts) => live && setTotal(counts.total),
      // A mark is a hint: a failed count keeps the last one rather than shouting.
      () => undefined,
    );
    return () => {
      live = false;
    };
  }, [recount]);
  useLiveSignal(["leads", "lead"], () => setRecount((n) => n + 1), 400);
  useLiveSignal(["places", "experiments", "sources", "bookings"], (signals) => {
    const events = signals.flatMap((s): ChangedEvent[] => (s.kind === "changed" ? [s.event] : []));
    if (events.length > 0) setUnseen((u) => noteChanges(u, events, at));
  });

  return {
    leads: newLeads(total, seen),
    away: focused ? 0 : newLeads(total, awayFrom),
    places: unseen.places.length,
    experiments: unseen.experiments,
    sources: unseen.sources,
    bookings: unseen.bookings,
  };
}
