/*
 * The cabinet's list treatment on top of the kit's Table, which carries the
 * borders, the row hover, the selected row (`data-state="selected"`) and the
 * scrolling wrapper. These are the same strings as the cabinet's
 * `views/admin/lib/table.ts`. lib 9eac780 ("add card tables") meant to move
 * them into the kit as `<Table variant="card">` and `TableCard`, but uikit
 * 0.26.0 as published has neither (its Table takes no variant, and
 * `ListRows` ships in dist without an export), so they stay here until a
 * release does.
 */

/** The tracked-uppercase header of every list. */
export const TABLE_HEAD = "text-xs font-medium uppercase tracking-wide text-ink-soft";

/** Cell padding for a table edge to edge in a `p-0` card: the kit's `p-2` assumes padding around it. */
export const EDGE_CELL = "px-5 py-3";
