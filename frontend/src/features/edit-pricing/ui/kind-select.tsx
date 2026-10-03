"use client";

import { Field, FieldLabel, Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@evinvest/uikit";

import { useControlSize } from "@/shared/ui/touch";

import type { FieldErrors } from "../model/errors";
import { domIdOf } from "../model/fields";
import { FieldMessages } from "./field-messages";

export interface KindSelectProps<K extends string> {
  field: string;
  label: string;
  value: K;
  options: readonly { value: K; label: string }[];
  onChange: (value: K) => void;
  errors: FieldErrors;
}

/** A closed choice of the model (an input's kind, a need's): the kit's Select, its reasons under it. */
export function KindSelect<K extends string>({ field, label, value, options, onChange, errors }: KindSelectProps<K>) {
  const size = useControlSize();
  const shown = errors.byField.get(field);
  const id = domIdOf(field);
  return (
    <Field className="flex min-w-0 flex-col gap-1" data-invalid={shown ? true : undefined}>
      <FieldLabel htmlFor={id}>{label}</FieldLabel>
      <Select value={value} onValueChange={(v) => onChange(options.find((o) => o.value === v)?.value ?? value)}>
        <SelectTrigger id={id} size={size} className="w-full" aria-invalid={shown ? true : undefined}>
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          {options.map((o) => (
            <SelectItem key={o.value} value={o.value}>
              {o.label}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>
      <FieldMessages shown={shown} />
    </Field>
  );
}
