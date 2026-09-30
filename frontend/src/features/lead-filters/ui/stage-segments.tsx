"use client";

import { ToggleGroup, ToggleGroupItem } from "@evinvest/uikit";

import type { Stage } from "@/entities/lead";
import { useT } from "@/shared/i18n";

import { SEGMENTS } from "../model/params";

/** "New · In progress · Quotes": the stages an operator clears from a phone. Tapping the lit one shows all. */
export function StageSegments({ stage, onChange }: { stage: Stage | null; onChange: (stage: Stage | null) => void }) {
  const t = useT();
  const value = SEGMENTS.some((s) => s.stage === stage) ? (stage ?? "") : "";
  return (
    <ToggleGroup
      type="single"
      variant="outline"
      size="xl"
      aria-label={t("filter.stage")}
      className="w-full"
      value={value}
      onValueChange={(v) => onChange(typeof v === "string" ? (SEGMENTS.find((s) => s.stage === v)?.stage ?? null) : null)}
    >
      {SEGMENTS.map((s) => (
        <ToggleGroupItem key={s.stage} value={s.stage} className="min-w-0 flex-1 truncate">
          {t(s.key)}
        </ToggleGroupItem>
      ))}
    </ToggleGroup>
  );
}
