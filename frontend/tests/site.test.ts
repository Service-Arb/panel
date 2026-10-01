import { describe, expect, it } from "vitest";

import { intentToLead, sourceBreakdown } from "@/views/overview/model/site";

describe("visits by source", () => {
  it("name the top three and sum the rest", () => {
    const b = sourceBreakdown({ google: 400, direct: 120, bing: 30, facebook: 20, "chatgpt.com": 5 });
    expect(b?.top.map((s) => s.source)).toEqual(["google", "direct", "bing"]);
    expect(b?.others).toBe(25);
  });

  it("have no 'other' when three or fewer brought visits", () => {
    expect(sourceBreakdown({ google: 4, direct: 4, none: 0 })).toEqual({ top: [{ source: "direct", n: 4 }, { source: "google", n: 4 }], others: 0 });
  });

  it("are not shown when one source brought everything", () => {
    expect(sourceBreakdown({ google: 300, direct: 0 })).toBeNull();
    expect(sourceBreakdown({})).toBeNull();
  });
});

describe("intent → lead across the edge", () => {
  it("is the two counts, never a percent, however many intents", () => {
    expect(intentToLead(4, 17)).toEqual({ leads: 4, intents: 17 });
    expect(intentToLead(430, 960)).toEqual({ leads: 430, intents: 960 });
    expect(intentToLead(50, 40)).toEqual({ leads: 50, intents: 40 });
  });

  it("is nothing when no intent was counted", () => {
    expect(intentToLead(5, 0)).toBeNull();
  });
});
