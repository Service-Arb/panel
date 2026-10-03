/**
 * How the site priced the need (FORM-VARIANTS-SPEC.md): `quote` asks for one,
 * `estimate` computed it from the visitor's choices, `fixed` is a set price.
 * `null` is a lead from before the variants, or one entered by hand.
 */
export const FLOWS = ["quote", "estimate", "fixed"] as const;
export type Flow = (typeof FLOWS)[number];

/** The variants' prices are EUR TTC: the contract has no currency field yet. */
export const QUOTE_CURRENCY = "EUR";

/** A price the site committed to: only `estimate` and `fixed` carry one. */
export function quotedPrice(lead: { flow: Flow | null; quoted_cents: number | null }): number | null {
  return lead.flow === "estimate" || lead.flow === "fixed" ? lead.quoted_cents : null;
}
