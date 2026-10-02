"use client";

import { Button, Field, FieldDescription, FieldError, FieldLabel, Input, ToggleGroup, ToggleGroupItem } from "@evinvest/uikit";
import { X } from "lucide-react";
import { useId } from "react";

import { DAYS, formatDays } from "@/entities/place";
import { type T, useT } from "@/shared/i18n";
import { useButtonSize, useControlSize } from "@/shared/ui/touch";

import { type HoursDraftRow, type RowProblem, normaliseTime, rowNotes, rowProblems } from "../model/hours-draft";

function problemText(p: RowProblem, t: T): string {
  switch (p.kind) {
    case "no_days":
      return t("placeSettings.hours.noDays");
    case "bad_opens":
    case "bad_closes":
      return t("placeSettings.hours.badTime");
    case "same_time":
      return t("placeSettings.hours.sameTime");
    case "overlap":
      return t("placeSettings.hours.overlap", { days: formatDays(p.days, t) });
  }
}

export interface HoursRowProps {
  rows: readonly HoursDraftRow[];
  row: HoursDraftRow;
  onChange: (patch: Partial<Omit<HoursDraftRow, "key">>) => void;
  onRemove: () => void;
}

/** One row: the days it covers, then when it opens and closes. */
export function HoursRowEditor({ rows, row, onChange, onRemove }: HoursRowProps) {
  const t = useT();
  const id = useId();
  const size = useControlSize();
  const button = useButtonSize();
  const problems = rowProblems(rows, row);
  // Two "bad time" lines would say the same thing twice.
  const reasons = [...new Set(problems.map((p) => problemText(p, t)))];
  return (
    <fieldset className="flex flex-col gap-2 rounded-md border border-border p-3">
      <legend className="sr-only">{t("placeSettings.field.hours")}</legend>
      <ToggleGroup
        type="multiple"
        variant="outline"
        size={size}
        aria-label={t("placeSettings.hours.days")}
        className="flex-wrap"
        value={row.days}
        onValueChange={(v) => onChange({ days: DAYS.filter((d) => (Array.isArray(v) ? v : [v]).includes(d)) })}
      >
        {DAYS.map((d) => (
          <ToggleGroupItem key={d} value={d} aria-label={d}>
            {t(`day.short.${d}`)}
          </ToggleGroupItem>
        ))}
      </ToggleGroup>
      <div className="flex items-end gap-2">
        <TimeInput id={`${id}-opens`} label={t("placeSettings.hours.opens")} value={row.opens} size={size} onChange={(opens) => onChange({ opens })} />
        <TimeInput id={`${id}-closes`} label={t("placeSettings.hours.closes")} value={row.closes} size={size} onChange={(closes) => onChange({ closes })} />
        <Button type="button" variant="ghost" size={button("md")} className="ml-auto" aria-label={t("placeSettings.hours.remove")} onClick={onRemove}>
          <X aria-hidden="true" />
        </Button>
      </div>
      {rowNotes(row).includes("overnight") && <FieldDescription>{t("placeSettings.hours.overnight")}</FieldDescription>}
      {reasons.map((r) => (
        <FieldError key={r}>{r}</FieldError>
      ))}
    </fieldset>
  );
}

/** Text, not `type="time"`: the browser's own picker ignores the kit's tokens and differs per OS. */
function TimeInput({ id, label, value, size, onChange }: { id: string; label: string; value: string; size: "md" | "lg"; onChange: (v: string) => void }) {
  return (
    <Field className="flex flex-col gap-1">
      <FieldLabel htmlFor={id}>{label}</FieldLabel>
      <Input id={id} size={size} className="w-24 tabular-nums" inputMode="numeric" maxLength={5} placeholder="08:00" value={value} onChange={(e) => onChange(e.target.value)} onBlur={(e) => onChange(normaliseTime(e.target.value))} />
    </Field>
  );
}
