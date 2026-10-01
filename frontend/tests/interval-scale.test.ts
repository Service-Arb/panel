import { describe, expect, it } from "vitest";

import { AXIS_CAP, axisFor, placeInterval } from "@/entities/experiment";

describe("the interval's axis", () => {
  it("is the smallest step that holds every interval of the experiment", () => {
    expect(axisFor([{ low: -1, high: 4 }, { low: 0.9, high: 2.7 }])).toBe(5);
    expect(axisFor([{ low: -0.4, high: 0.6 }])).toBe(1);
  });

  it("stops at the cap, so one wide interval does not flatten the rest", () => {
    expect(axisFor([{ low: -35, high: 60 }])).toBe(AXIS_CAP);
  });

  it("has a width even with nothing to draw", () => {
    expect(axisFor([])).toBe(1);
  });
});

describe("an interval on the axis", () => {
  it("puts zero in the middle and the ends where their values are", () => {
    const p = placeInterval({ low: -2, high: 4 }, 1, 5, 160);
    expect(p.zero).toBe(80);
    expect(p.from).toBeCloseTo(48);
    expect(p.to).toBeCloseTo(144);
    expect(p.estimate).toBeCloseTo(96);
    expect([p.clippedLow, p.clippedHigh]).toEqual([false, false]);
  });

  it("is cut at the edge, and says so, when it runs past the axis", () => {
    const p = placeInterval({ low: -30, high: 8 }, null, 20, 160);
    expect(p.from).toBe(0);
    expect(p.clippedLow).toBe(true);
    expect(p.to).toBeCloseTo(112);
    expect(p.clippedHigh).toBe(false);
    expect(p.estimate).toBeNull();
    expect(placeInterval({ low: 5, high: 50 }, 30, 20, 160)).toMatchObject({ to: 160, estimate: 160, clippedHigh: true });
  });
});
