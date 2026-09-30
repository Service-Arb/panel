import { describe, expect, it } from "vitest";

import { managesSources, startRouteFor } from "@/entities/session/model/role";

describe("the start screen", () => {
  it("is the new leads for an operator", () => {
    expect(startRouteFor("operator")).toBe("/leads?stage=created");
  });

  it("is the overview for an admin", () => {
    expect(startRouteFor("admin")).toBe("/overview");
  });

  it("offers sources to admins only", () => {
    expect(managesSources("admin")).toBe(true);
    expect(managesSources("operator")).toBe(false);
  });
});
