import { describe, expect, it } from "vitest";

import { leadParser } from "@/entities/lead/model/lead";
import { quotedPrice } from "@/entities/lead/model/pricing";
import { flowFilterOf, leadFilterFrom, narrows, paramsWith } from "@/features/lead-filters/model/params";
import { formatCents, formatDay } from "@/shared/lib/format";
import { parse } from "@/shared/lib/parse";

const row = { brand: "vifnet", lead_id: "l1", stage: "created", manual: false, last_event_at: "2026-10-03T10:00:00Z" };

describe("a lead's form variant", () => {
  it("reads as nulls on a lead from before the variants", () => {
    const l = parse(leadParser, row);
    expect([l.flow, l.quoted_cents, l.pricing_valid_from, l.estimate_inputs]).toEqual([null, null, null, null]);
  });

  it("reads an estimate with its price, model day and inputs", () => {
    const l = parse(leadParser, { ...row, flow: "estimate", quoted_cents: 12_350, pricing_valid_from: "2026-09-28", estimate_inputs: { zone: "paris-intra", bedrooms: "2" } });
    expect(l.flow).toBe("estimate");
    expect(quotedPrice(l)).toBe(12_350);
    expect(l.estimate_inputs).toEqual({ zone: "paris-intra", bedrooms: "2" });
  });

  it("refuses a variant or money outside the contract", () => {
    expect(() => parse(leadParser, { ...row, flow: "auction" })).toThrow(/\$\.flow/);
    expect(() => parse(leadParser, { ...row, quoted_cents: 12.5 })).toThrow(/quoted_cents/);
    expect(() => parse(leadParser, { ...row, quoted_cents: -1 })).toThrow(/quoted_cents/);
    expect(() => parse(leadParser, { ...row, pricing_valid_from: "2026-09-28T00:00:00Z" })).toThrow(/pricing_valid_from/);
    expect(() => parse(leadParser, { ...row, estimate_inputs: { zone: 3 } })).toThrow(/estimate_inputs\.zone/);
  });

  it("shows a price only for estimate and fixed", () => {
    expect(quotedPrice({ flow: "fixed", quoted_cents: 8_900 })).toBe(8_900);
    expect(quotedPrice({ flow: "quote", quoted_cents: 8_900 })).toBeNull();
    expect(quotedPrice({ flow: null, quoted_cents: null })).toBeNull();
  });
});

describe("the flow filter", () => {
  it("takes only the API's words, and is every lead otherwise (the API answers 400 to the rest)", () => {
    expect(leadFilterFrom(new URLSearchParams("flow=estimate")).flow).toBe("estimate");
    expect(flowFilterOf("fixed")).toBe("fixed");
    expect(flowFilterOf("Estimate")).toBeNull();
    expect(flowFilterOf("")).toBeNull();
    expect(leadFilterFrom(new URLSearchParams("flow=auction")).flow).toBeNull();
  });

  it("goes into the URL and back out, and counts as a filter", () => {
    const on = paramsWith(new URLSearchParams("stage=created"), { flow: "fixed" });
    expect(on.toString()).toBe("stage=created&flow=fixed");
    expect(narrows(leadFilterFrom(on), ["flow"])).toBe(true);
    expect(paramsWith(on, { flow: null }).toString()).toBe("stage=created");
    expect(narrows(leadFilterFrom(new URLSearchParams()))).toBe(false);
  });
});

describe("money from cents", () => {
  // Intl puts a narrow no-break space in some locales; compared on plain spaces.
  const plain = (s: string) => s.replace(/[  ]/g, " ");

  it("shows whole euros bare and cents only when there are some", () => {
    expect(formatCents(8_900, "EUR", "en")).toBe("€89");
    expect(formatCents(8_950, "EUR", "en")).toBe("€89.50");
    expect(plain(formatCents(123_405, "EUR", "ru"))).toBe("1 234,05 €");
    expect(plain(formatCents(5, "EUR", "fr"))).toBe("0,05 €");
  });

  it("stays exact past what a float holds", () => {
    expect(formatCents(900_719_925_474_099, "EUR", "en")).toBe("€9,007,199,254,740.99");
  });
});

describe("the price model's day", () => {
  it("shows as that day in every zone", () => {
    expect(formatDay("2026-09-28", "en")).toBe("Sep 28, 2026");
  });
});
