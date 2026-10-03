import type { Flash } from "../model/use-lead-list";

/**
 * The row's `data-flash` (globals.css): two names for "changed" that alternate,
 * because a CSS animation replays only when its name changes — a second change
 * to the same row must flash again.
 */
export function flashAttr(flash: Flash | undefined): "new" | "odd" | "even" | undefined {
  if (flash === undefined) return undefined;
  if (flash === "new") return "new";
  return flash % 2 === 1 ? "odd" : "even";
}
