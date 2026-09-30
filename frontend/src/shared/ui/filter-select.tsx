"use client";

import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@evinvest/uikit";

const ALL = "__all__";

export interface FilterOption {
  value: string;
  label: string;
}

/** A filter over one field: "all" or one value. `null` is "all". */
export function FilterSelect({ label, allLabel, value, options, onChange }: { label: string; allLabel: string; value: string | null; options: FilterOption[]; onChange: (v: string | null) => void }) {
  return (
    <Select value={value ?? ALL} onValueChange={(v) => onChange(v === ALL ? null : v)}>
      <SelectTrigger size="sm" aria-label={label} className="min-w-36">
        <SelectValue />
      </SelectTrigger>
      <SelectContent>
        <SelectItem value={ALL}>{allLabel}</SelectItem>
        {options.map((o) => (
          <SelectItem key={o.value} value={o.value}>
            {o.label}
          </SelectItem>
        ))}
      </SelectContent>
    </Select>
  );
}
