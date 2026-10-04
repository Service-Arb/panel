import { describe, expect, it } from "vitest";

import { offeredBrands } from "@/views/places/model/brands";

const source = (brands: string[], revoked_at: string | null = null) => ({ brands, revoked_at });

describe("offeredBrands", () => {
  it("offers a brand an active source writes for before it has any place", () => {
    expect(offeredBrands([{ brand: "aquafix" }], [source(["aquafix"]), source(["vifnet"])])).toEqual(["aquafix", "vifnet"]);
  });

  it("leaves out a brand only a revoked source wrote for", () => {
    expect(offeredBrands([], [source(["oldbrand"], "2026-10-01T00:00:00Z")])).toEqual([]);
  });

  it("keeps every place's brand once, sorted, with no sources", () => {
    expect(offeredBrands([{ brand: "vifnet" }, { brand: "aquafix" }, { brand: "vifnet" }], [])).toEqual(["aquafix", "vifnet"]);
  });
});
