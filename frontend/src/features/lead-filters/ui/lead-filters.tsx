"use client";

import { Label, Switch } from "@evinvest/uikit";
import { useId } from "react";

import { type LeadFilter, STAGES } from "@/entities/lead";
import { useT } from "@/shared/i18n";
import { FilterSelect } from "@/shared/ui/filter-select";

import { stageOrNull } from "../model/params";

/** Stage, brand, location and "overdue only" — the questions the queue is sorted by. */
export function LeadFilters({ filter, brands, locations, onChange }: { filter: LeadFilter; brands: string[]; locations: string[]; onChange: (patch: Partial<LeadFilter>) => void }) {
  const t = useT();
  const id = useId();
  return (
    <div className="flex flex-wrap items-center gap-2">
      <FilterSelect
        label={t("filter.stage")}
        allLabel={t("filter.stage.all")}
        value={filter.stage}
        options={STAGES.map((s) => ({ value: s, label: t(`stage.${s}`) }))}
        onChange={(v) => onChange({ stage: stageOrNull(v) })}
      />
      <FilterSelect label={t("filter.brand")} allLabel={t("filter.brand.all")} value={filter.brand} options={brands.map((b) => ({ value: b, label: b }))} onChange={(brand) => onChange({ brand, location: null })} />
      <FilterSelect label={t("filter.location")} allLabel={t("filter.location.all")} value={filter.location} options={locations.map((l) => ({ value: l, label: l }))} onChange={(location) => onChange({ location })} />
      <div className="flex items-center gap-2 px-1">
        <Switch id={`${id}-overdue`} checked={filter.overdue} onCheckedChange={(overdue) => onChange({ overdue })} />
        <Label htmlFor={`${id}-overdue`}>{t("filter.overdue")}</Label>
      </div>
    </div>
  );
}
