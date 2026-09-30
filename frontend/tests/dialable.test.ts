import { describe, expect, it } from "vitest";

import { dialable } from "@/entities/lead/model/lead";

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
