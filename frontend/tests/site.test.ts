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
  it("is two counts, never a percent, on fewer intents than the minimum sample", () => {
    expect(intentToLead(4, 17, 30)).toEqual({ kind: "counts", leads: 4, intents: 17 });
  });

  it("is a whole percent from the minimum sample on", () => {
    expect(intentToLead(43, 96, 30)).toEqual({ kind: "percent", percent: 45, leads: 43, intents: 96 });
  });

  it("is two counts when leads outnumber intents (calls with no click)", () => {
    expect(intentToLead(50, 40, 30)).toEqual({ kind: "counts", leads: 50, intents: 40 });
  });

  it("is nothing when no intent was counted", () => {
    expect(intentToLead(5, 0, 30)).toBeNull();
  });
});
