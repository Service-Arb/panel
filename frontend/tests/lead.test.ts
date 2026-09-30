import { describe, expect, it } from "vitest";

import { dialable, slaAt } from "@/entities/lead/model/lead";

describe("the number a call dials", () => {
  it("keeps only + and digits", () => {
    expect(dialable("+33 6 00-00.00 01")).toBe("+33600000001");
    expect(dialable("06 12 34 56 78 (evenings)")).toBe("0612345678");
  });

  it("is none for fewer than six digits or no number", () => {
    expect(dialable("12-34")).toBeNull();
    expect(dialable("call me")).toBeNull();
    expect(dialable("javascript:alert(1)")).toBeNull();
    expect(dialable(null)).toBeNull();
  });
});

describe("the SLA as time passes", () => {
  const sla = { waiting_since: "2026-09-30T10:00:00Z", waiting_seconds: 60, overdue: false };
  const at = (min: number) => Date.parse("2026-09-30T10:00:00Z") + min * 60_000;

  it("counts from waiting_since, not from when the page loaded", () => {
    expect(slaAt(sla, at(20))).toEqual({ seconds: 1200, overdue: false });
  });

  it("turns overdue past 30 minutes without a reload", () => {
    expect(slaAt(sla, at(31)).overdue).toBe(true);
  });
});
