import { describe, expect, it } from "vitest";

import { type Comparison, experimentsParser, formatPoints, readComparison } from "@/entities/experiment";
import { translator } from "@/shared/i18n/translate";
import { ParseError, parse } from "@/shared/lib/parse";
import { formatShare } from "@/shared/lib/share";

const en = translator("en");
const intents = { phone: 1, whatsapp: 0, form_open: 3, booking: 0 };
const small = { n: 2, of: 64, percent: null, small_sample: true };
const answer = {
  from: "2026-09-02", to: "2026-10-01", brand: null, min_sample: 30, min_exposures: 100, confidence: 0.95, z: 1.96, interval: "newcombe_hybrid_score",
  source: { source: "posthog", kind: "aggregate", imported_at: "2026-10-01T09:00:00Z" },
  experiments: [{
    brand: "aquafix", experiment: "hero_cta", first_day: "2026-09-10", last_day: "2026-10-01", control: "a",
    variants: [
      { variant: "a", control: true, exposures: 412, leads: 9, intents, rates: { lead: { n: 9, of: 412, percent: 2, small_sample: false }, contact: { n: 26, of: 412, percent: 6, small_sample: false } }, vs_control: null },
      { variant: "c", control: false, exposures: 64, leads: 2, intents, rates: { lead: small, contact: small }, vs_control: {
        lead: { difference: null, insufficient: true, reason: "small_sample" },
        contact: { difference: null, insufficient: true, reason: "small_sample" },
      } },
    ],
  }],
};

describe("the experiments answer", () => {
  it("is read with the control first and no comparison of its own", () => {
    const e = parse(experimentsParser, answer).experiments[0];
    expect(e?.variants.map((v) => [v.variant, v.control])).toEqual([["a", true], ["c", false]]);
    expect(e?.variants[0]?.vs_control).toBeNull();
    expect(e?.variants[1]?.vs_control?.lead.reason).toBe("small_sample");
  });

  it("refuses a reason it does not know", () => {
    const bad = structuredClone(answer);
    bad.experiments[0]!.variants[1]!.vs_control!.lead.reason = "winner";
    expect(() => parse(experimentsParser, bad)).toThrow(ParseError);
  });

  it("shows a small arm's rate as n of m, never a percent", () => {
    const rate = parse(experimentsParser, answer).experiments[0]!.variants[1]!.rates.lead;
    expect(formatShare(rate, en)).toBe("2 of 64");
  });
});

describe("a comparison on screen", () => {
  const interval = { estimate: 1, low: -1, high: 4, decimals: 0 };

  it("is only a badge while an arm is too small: no interval, no estimate", () => {
    const r = readComparison({ difference: null, insufficient: true, reason: "small_sample" });
    expect(r).toEqual({ interval: null, showEstimate: false, badge: "small_sample" });
  });

  it("keeps the interval but drops the estimate while the interval includes zero", () => {
    const r = readComparison({ difference: interval, insufficient: true, reason: "interval_includes_zero" });
    expect(r.interval).toEqual(interval);
    expect(r.showEstimate).toBe(false);
    expect(r.badge).toBe("interval_includes_zero");
  });

  it("names a reason even if the backend left it out", () => {
    const noReason: Comparison = { difference: null, insufficient: true, reason: null };
    expect(readComparison(noReason).badge).toBe("small_sample");
    expect(readComparison({ ...noReason, difference: interval }).badge).toBe("interval_includes_zero");
  });

  it("gives the estimate, and no badge, once the interval clears zero", () => {
    const r = readComparison({ difference: { estimate: 2, low: 0.9, high: 2.7, decimals: 1 }, insufficient: false, reason: null });
    expect(r).toMatchObject({ showEstimate: true, badge: null });
  });

  it("writes points to the backend's decimals, signed, with a true minus", () => {
    expect(formatPoints(-2, 0, "en")).toBe("−2");
    expect(formatPoints(10, 0, "en")).toBe("+10");
    expect(formatPoints(0.4, 1, "ru")).toBe("+0,4");
    expect(formatPoints(0, 1, "en")).toBe("0.0");
  });
});
