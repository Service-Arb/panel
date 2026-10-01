import { describe, expect, it } from "vitest";

import { leadCountsParser } from "@/entities/lead/model/lead";
import { leadFilterFrom, paramsWith, segmentLabels } from "@/features/lead-filters/model/params";
import { translator } from "@/shared/i18n/translate";
import { parse } from "@/shared/lib/parse";

const en = translator("en");
const ru = translator("ru");

const answer = {
  stages: { created: 3, contacted: 1, quoted: 2, won: 4, completed: 0, paid: 7, lost: 5 },
  overdue: 2,
  total: 22,
};

describe("lead counts", () => {
  it("parse /leads/counts with every stage", () => {
    expect(parse(leadCountsParser, answer).stages.quoted).toBe(2);
  });

  it("refuse an answer missing a stage", () => {
    const short = Object.fromEntries(Object.entries(answer.stages).filter(([stage]) => stage !== "lost"));
    expect(() => parse(leadCountsParser, { ...answer, stages: short })).toThrow(/\$\.stages\.lost/);
  });
});

describe("the stage segments", () => {
  it("carry the count of the stage each lists", () => {
    const counts = parse(leadCountsParser, answer);
    expect(segmentLabels(counts, ru).map((s) => s.label)).toEqual(["Новые · 3", "В работе · 1", "Сметы · 2"]);
    expect(segmentLabels(counts, en).map((s) => s.stage)).toEqual(["created", "contacted", "quoted"]);
  });

  it("show a zero as a zero, and the bare name until the counts arrive", () => {
    const none = parse(leadCountsParser, { ...answer, stages: { ...answer.stages, created: 0 } });
    expect(segmentLabels(none, en)[0]?.label).toBe("New · 0");
    expect(segmentLabels(null, en).map((s) => s.label)).toEqual(["New", "In progress", "Quotes"]);
  });
});

describe("the created range in the URL", () => {
  it("round-trips UTC days and drops anything else", () => {
    const params = paramsWith(new URLSearchParams(), { createdFrom: "2026-09-01", createdTo: "2026-09-30" });
    expect(params.toString()).toBe("created_from=2026-09-01&created_to=2026-09-30");
    expect(leadFilterFrom(params)).toMatchObject({ createdFrom: "2026-09-01", createdTo: "2026-09-30" });
    expect(leadFilterFrom(new URLSearchParams("created_from=yesterday")).createdFrom).toBeNull();
  });
});
