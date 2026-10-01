import { describe, expect, it } from "vitest";

import { aggregateParser, funnelByLocationParser, funnelParser } from "@/entities/funnel";
import { ParseError, parse } from "@/shared/lib/parse";

const channels = { phone: 3, whatsapp: 1, form_open: 4, booking: 0 };
const aggregate = {
  stages: [
    { stage: "site.visit", total: 120, by_source: { google: 80, direct: 40 }, days: [{ day: "2026-09-30", n: 120 }] },
    { stage: "contact.intent", total: 8, by_channel: channels, days: [{ day: "2026-09-30", n: 8, by_channel: channels }] },
  ],
};
const source = { source: "posthog", kind: "aggregate", imported_at: "2026-10-01T09:00:00Z" };
const share = { n: 0, of: 0, percent: null, small_sample: true };
const slice = { stages: [], lost: share, manual: share, payments: [], aggregate };

describe("the funnel's day counts", () => {
  it("are read beside the leads, with where they came from", () => {
    const f = parse(funnelParser, { from: "2026-09-02", to: "2026-10-01", brand: null, min_sample: 30, aggregate_source: source, ...slice });
    expect(f.aggregate.visits.total).toBe(120);
    expect(f.aggregate.visits.by_source).toEqual({ google: 80, direct: 40 });
    expect(f.aggregate.intents.by_channel.phone).toBe(3);
    expect(f.aggregate_source.imported_at).toBe("2026-10-01T09:00:00Z");
  });

  it("are unknown, not zero, before the first import", () => {
    const f = parse(funnelParser, { from: "a", to: "b", brand: null, min_sample: 30, aggregate_source: { ...source, imported_at: null }, ...slice });
    expect(f.aggregate_source.imported_at).toBeNull();
  });

  it("come with every location row", () => {
    const row = { brand: "aquafix", location: "villeurbanne", ...slice };
    const f = parse(funnelByLocationParser, { from: "a", to: "b", brand: null, min_sample: 30, aggregate_source: source, by: "location", locations: [row] });
    expect(f.locations[0]?.aggregate.visits.total).toBe(120);
  });

  it("skip a stage the front does not read yet (Maps comes later)", () => {
    const withMaps = { stages: [{ stage: "maps.impression", total: 9000 }, ...aggregate.stages] };
    expect(parse(aggregateParser, withMaps).intents.total).toBe(8);
  });

  it("refuse an answer that lost a stage or a channel", () => {
    expect(() => parse(aggregateParser, { stages: [aggregate.stages[0]] })).toThrow(ParseError);
    const noBooking = { ...aggregate.stages[1], by_channel: { phone: 1, whatsapp: 0, form_open: 0 } };
    expect(() => parse(aggregateParser, { stages: [aggregate.stages[0], noBooking] })).toThrow(/booking/);
  });
});
