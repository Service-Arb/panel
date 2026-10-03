/*
 * The cabinet's list treatment on top of the kit's Table, which carries the
 * borders, the row hover, the selected row (`data-state="selected"`) and the
 * scrolling wrapper. These are the same strings as the cabinet's
 * `views/admin/lib/table.ts`; they belong in the kit as a Table variant, and
 * both fronts should take them from there once it ships one.
 */

/** The tracked-uppercase header of every list. */
export const TABLE_HEAD = "text-xs font-medium uppercase tracking-wide text-ink-soft";

/** Cell padding for a table edge to edge in a `p-0` card: the kit's `p-2` assumes padding around it. */
export const EDGE_CELL = "px-5 py-3";
