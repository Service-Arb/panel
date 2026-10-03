import { describe, expect, it } from "vitest";

import type { Role } from "@/entities/session";
import type { T } from "@/shared/i18n";
import { panelNav } from "@/views/shell/ui/nav-items";
import type { Marks } from "@/views/shell/model/use-marks";

const t: T = (key) => key;
const marks: Marks = { leads: 0, away: 0, places: 0, experiments: false, sources: false, bookings: false };
const ids = (role: Role) => Object.fromEntries(panelNav(t, role, marks).groups.map((g) => [g.id, g.items.map((i) => i.id)]));

describe("the rail", () => {
  it("files Grafana under Analysis, as an external link, for every role", () => {
    for (const role of ["operator", "admin"] as const) {
      expect(ids(role).analysis).toEqual(["experiments", "grafana"]);
      const grafana = panelNav(t, role, marks).groups.flatMap((g) => g.items).find((i) => i.id === "grafana");
      expect(grafana?.external).toBe(true);
      expect(grafana?.trailing).toBeDefined();
    }
  });

  it("has no Account group: the profile block is the rail's footer, not a nav group", () => {
    expect(Object.keys(ids("admin"))).toEqual(["work", "analysis", "admin"]);
    expect(Object.keys(ids("operator"))).toEqual(["work", "analysis"]);
  });
});
