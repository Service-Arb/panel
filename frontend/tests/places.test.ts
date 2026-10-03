import { describe, expect, it } from "vitest";

import { funnelByLocationParser, funnelParser } from "@/entities/funnel/model/funnel";
import { brandsOf, locationsOf, placesParser } from "@/entities/place/model/place";
import { translator } from "@/shared/i18n/translate";
import { parse } from "@/shared/lib/parse";
import { formatShare } from "@/shared/lib/share";
import { placeRows } from "@/views/places/model/rows";

const en = translator("en");

const small = (n: number, of: number) => ({ n, of, percent: null, small_sample: true });
const big = (n: number, of: number, percent: number) => ({ n, of, percent, small_sample: false });

/** A slice's steps as the backend answers them: `reached` and the share of the slice's leads. */
function stages(leads: number, reached: Record<string, number>, share: (n: number, of: number) => object) {
  return ["created", "contacted", "quoted", "won", "completed", "paid"].map((stage, i) => ({
    stage,
    reached: stage === "created" ? leads : (reached[stage] ?? 0),
    of_previous: i === 0 ? null : share(reached[stage] ?? 0, leads),
    of_leads: share(stage === "created" ? leads : (reached[stage] ?? 0), leads),
  }));
}

const lyon = { brand: "aquafix", location: "lyon-7", stages: stages(3, { contacted: 2, won: 1, paid: 1 }, small), lost: small(0, 3), manual: small(0, 3), payments: [{ currency: "EUR", billed: 23_100, commission: 2_310, count: 1 }] };
const range = { from: "2026-09-02", to: "2026-10-01", brand: null, min_sample: 30 };

const byLocation = {
  ...range,
  by: "location",
  locations: [
    lyon,
    { brand: "aquafix", location: null, stages: stages(1, {}, small), lost: small(0, 1), manual: small(1, 1), payments: [] },
    {
      brand: "vifnet",
      location: "paris-11",
      stages: stages(40, { contacted: 30, won: 12, paid: 10 }, (n, of) => big(n, of, Math.round((n * 100) / of))),
      lost: big(4, 40, 10),
      manual: big(0, 40, 0),
      payments: [],
    },
  ],
};

describe("the per-location funnel", () => {
  it("parses /funnel?by=location, a location of null included", () => {
    const funnel = parse(funnelByLocationParser, byLocation);
    expect(funnel.min_sample).toBe(30);
    expect(funnel.locations.map((l) => l.location)).toEqual(["lyon-7", null, "paris-11"]);
    expect(funnel.locations[0]?.payments).toEqual([{ currency: "EUR", billed: 23_100, commission: 2_310, count: 1 }]);
  });

  it("refuses the whole funnel's answer where the slices are expected", () => {
    const whole = { ...range, stages: lyon.stages, lost: lyon.lost, manual: lyon.manual, payments: [] };
    expect(() => parse(funnelByLocationParser, whole)).toThrow(/\$\.by/);
  });

  it("reads payments in the whole funnel, one row per currency", () => {
    const payments = [...lyon.payments, { currency: "GBP", billed: 9_000, commission: 900, count: 2 }];
    const funnel = parse(funnelParser, { ...range, stages: lyon.stages, lost: lyon.lost, manual: lyon.manual, payments });
    expect(funnel.payments.map((p) => p.currency)).toEqual(["EUR", "GBP"]);
  });
});

describe("location cards", () => {
  const rows = placeRows(parse(funnelByLocationParser, byLocation).locations);

  it("are busiest first, the leads with no location kept as their own row", () => {
    expect(rows.map((r) => [r.brand, r.location, r.leads])).toEqual([
      ["vifnet", "paris-11", 40],
      ["aquafix", "lyon-7", 3],
      ["aquafix", null, 1],
    ]);
  });

  it("show a small sample as n of m and never a percent", () => {
    const lyon = rows.find((r) => r.location === "lyon-7");
    expect(lyon?.steps.map((s) => s.stage)).toEqual(["contacted", "won", "paid"]);
    const shown = lyon?.steps.map((s) => formatShare(s.share, en));
    expect(shown).toEqual(["2 of 3", "1 of 3", "1 of 3"]);
    expect(shown?.join(" ")).not.toMatch(/%/);
  });

  it("show the backend's percent once the sample is large enough", () => {
    expect(rows[0]?.steps.map((s) => formatShare(s.share, en))).toEqual(["75%", "30%", "25%"]);
  });

  it("carry each place's site-data flags, and list a place with no lead last", () => {
    const places = parse(placesParser, {
      places: [
        { brand: "aquafix", location: "lyon-7", last_lead_at: null, has_settings: true, withdrawn: false },
        { brand: "aquafix", location: "royat", last_lead_at: null, has_settings: false, withdrawn: true },
      ],
    }).places;
    const withSites = placeRows(parse(funnelByLocationParser, byLocation).locations, places);
    expect(withSites.map((r) => [r.location, r.leads, r.site])).toEqual([
      ["paris-11", 40, { hasSettings: false, withdrawn: false }],
      ["lyon-7", 3, { hasSettings: true, withdrawn: false }],
      [null, 1, null],
      ["royat", 0, { hasSettings: false, withdrawn: true }],
    ]);
  });
});

describe("the places the filters offer", () => {
  const places = parse(placesParser, {
    places: [
      { brand: "vifnet", location: "paris-11", last_lead_at: "2026-09-30T10:00:00Z" },
      { brand: "aquafix", location: "lyon-7", last_lead_at: null },
      { brand: "aquafix", location: "lyon-3", last_lead_at: "2026-09-29T10:00:00Z" },
    ],
  }).places;

  it("are the brands /places names, plus the one already chosen", () => {
    expect(brandsOf(places)).toEqual(["aquafix", "vifnet"]);
    expect(brandsOf(places, "newbrand")).toEqual(["aquafix", "newbrand", "vifnet"]);
  });

  it("are a brand's own locations, plus the one already chosen", () => {
    expect(locationsOf(places, "aquafix")).toEqual(["lyon-3", "lyon-7"]);
    expect(locationsOf(places, "aquafix", "lyon-9")).toEqual(["lyon-3", "lyon-7", "lyon-9"]);
    expect(locationsOf(places, null)).toEqual(["lyon-3", "lyon-7", "paris-11"]);
  });
});
