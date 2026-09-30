import { describe, expect, it } from "vitest";

import type { Lead, LeadPage } from "@/entities/lead/model/lead";
import { aggregatePlaces, loadWindow } from "@/views/places/model/aggregate";

function lead(over: Partial<Lead>): Lead {
  return {
    brand: "aquafix",
    lead_id: "L",
    location: "lyon-3",
    job_id: null,
    stage: "created",
    channel: "form",
    manual: false,
    created_at: "2026-09-20T10:00:00Z",
    contacted_at: null,
    quoted_at: null,
    won_at: null,
    completed_at: null,
    paid_at: null,
    lost_at: null,
    lost_reason: null,
    last_event_at: "2026-09-20T10:00:00Z",
    sla: null,
    pii: null,
    ...over,
  };
}

describe("locations counted in the browser", () => {
  it("count a stage as reached by its time or any later one, lost leads included", () => {
    const rows = aggregatePlaces(
      [
        lead({ lead_id: "a" }),
        lead({ lead_id: "b", stage: "lost", contacted_at: "2026-09-20T11:00:00Z", lost_at: "2026-09-21T00:00:00Z" }),
        lead({ lead_id: "c", stage: "paid", paid_at: "2026-09-25T00:00:00Z" }),
        lead({ lead_id: "d", location: "lyon-7" }),
        lead({ lead_id: "old", created_at: "2026-08-01T00:00:00Z" }),
      ],
      "2026-09-01T00:00:00.000Z",
    );
    expect(rows).toEqual([
      { brand: "aquafix", location: "lyon-3", leads: 3, contacted: 2, won: 1, paid: 1 },
      { brand: "aquafix", location: "lyon-7", leads: 1, contacted: 0, won: 0, paid: 0 },
    ]);
  });

  it("stop paging at the window's edge, and give up past the limit", async () => {
    const pages: Record<string, LeadPage> = {
      first: { leads: [lead({ created_at: "2026-09-29T00:00:00Z" })], next_cursor: "c2" },
      c2: { leads: [lead({ created_at: "2026-08-01T00:00:00Z" })], next_cursor: "c3" },
    };
    const seen: (string | null)[] = [];
    const fetchPage = async (cursor: string | null) => {
      seen.push(cursor);
      return pages[cursor ?? "first"] ?? { leads: [], next_cursor: null };
    };
    expect((await loadWindow(fetchPage, "2026-09-01T00:00:00Z")).complete).toBe(true);
    expect(seen).toEqual([null, "c2"]);

    const endless = async () => ({ leads: [lead({ created_at: "2026-09-29T00:00:00Z" })], next_cursor: "more" });
    expect((await loadWindow(endless, "2026-09-01T00:00:00Z", 3)).complete).toBe(false);
  });
});
