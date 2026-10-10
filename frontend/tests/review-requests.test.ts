import { describe, expect, it } from "vitest";

import { groupByWeek, isPartialWeek, reviewRequestsParser } from "@/entities/review-request/model/review-request";
import { parse } from "@/shared/lib/parse";

const share = (n: number, of: number, percent: number | null = null) => ({ n, of, percent, small_sample: percent === null });

describe("GET /review-requests", () => {
  const body = {
    from: "2026-09-12", to: "2026-10-11", brand: null, min_sample: 30, total: share(3, 8),
    weeks: [{ week: "2026-10-05", brand: "aquafix", location: null, share: share(3, 8) }],
  };

  it("reads the total and the weeks, a place that names no location as null", () => {
    expect(parse(reviewRequestsParser, body).weeks[0]).toMatchObject({ week: "2026-10-05", location: null, share: { n: 3, of: 8, percent: null, small_sample: true } });
  });

  it("refuses a week that is not a day", () => {
    expect(() => parse(reviewRequestsParser, { ...body, weeks: [{ ...body.weeks[0], week: "next monday" }] })).toThrow(/week/);
  });
});

describe("a week the period cuts", () => {
  it("is whole when the period holds Monday to Sunday", () => {
    expect(isPartialWeek("2026-10-05", "2026-10-01", "2026-10-31")).toBe(false);
    expect(isPartialWeek("2026-10-05", "2026-10-05", "2026-10-11")).toBe(false);
  });

  it("is partial when the period starts after its Monday or ends before its Sunday", () => {
    expect(isPartialWeek("2026-10-05", "2026-10-07", "2026-10-31")).toBe(true);
    expect(isPartialWeek("2026-10-05", "2026-09-01", "2026-10-09")).toBe(true);
  });
});

describe("the weeks as listed", () => {
  it("puts the newest first, each week's places together in order", () => {
    const row = (week: string, brand: string, location: string | null) => ({ week, brand, location, share: share(1, 2) });
    const groups = groupByWeek([row("2026-09-28", "aquafix", "lyon-3"), row("2026-10-05", "vifnet", "paris-11"), row("2026-10-05", "aquafix", "lyon-7"), row("2026-10-05", "aquafix", null)]);
    expect(groups.map((g) => g.week)).toEqual(["2026-10-05", "2026-09-28"]);
    expect(groups[0]!.rows.map((r) => `${r.brand}/${r.location}`)).toEqual(["aquafix/null", "aquafix/lyon-7", "vifnet/paris-11"]);
  });
});
