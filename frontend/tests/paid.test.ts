import { describe, expect, it } from "vitest";

import { paidLines } from "@/views/overview/model/paid";

// Intl puts a narrow no-break space in some locales; compare with plain spaces.
const plain = (s: string | null) => s?.replace(/\s/g, " ") ?? null;

describe("payments on the overview", () => {
  const payments = [
    { currency: "EUR", billed: 231_000, commission: 23_100, count: 3 },
    { currency: "GBP", billed: 9_050, commission: 0, count: 1 },
  ];

  it("keep a line per currency and never add them up", () => {
    const lines = paidLines(payments, "en");
    expect(lines.map((l) => l.currency)).toEqual(["EUR", "GBP"]);
    expect(lines.map((l) => plain(l.billed))).toEqual(["€2,310", "£91"]);
  });

  it("show whole units, rounded, in the reader's locale", () => {
    expect(plain(paidLines([{ currency: "EUR", billed: 231_049, commission: 99, count: 1 }], "ru")[0]?.billed ?? null)).toBe("2 310 €");
  });

  it("show the commission only where some was kept", () => {
    const lines = paidLines(payments, "en");
    expect(plain(lines[0]?.commission ?? null)).toBe("€231");
    expect(lines[1]?.commission).toBeNull();
  });

  it("are none when nothing was paid", () => {
    expect(paidLines([], "en")).toEqual([]);
  });
});
