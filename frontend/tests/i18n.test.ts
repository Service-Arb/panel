import { describe, expect, it } from "vitest";

import en from "../messages/en.json";
import ru from "../messages/ru.json";

describe("the catalogues", () => {
  it("have the same keys: none missing in ru, none that English lacks", () => {
    expect(Object.keys(ru).sort()).toEqual(Object.keys(en).sort());
  });

  it("have no empty strings", () => {
    for (const cat of [en, ru]) expect(Object.values(cat).filter((v) => v.trim() === "")).toEqual([]);
  });
});
