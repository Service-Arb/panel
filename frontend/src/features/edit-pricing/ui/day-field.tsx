"use client";

import { Button, Calendar, Field, FieldLabel, Popover, PopoverContent, PopoverTrigger } from "@evinvest/uikit";
import { CalendarDays } from "lucide-react";
import { useState } from "react";

import { isDay } from "@/entities/pricing";
import { useLocale, useT } from "@/shared/i18n";
import { formatDay } from "@/shared/lib/format";
import { useButtonSize } from "@/shared/ui/touch";

import type { FieldErrors } from "../model/errors";
import { describedByOf, domIdOf } from "../model/fields";
import { FieldMessages } from "./field-messages";

/** The Calendar works in local days; the model in `YYYY-MM-DD`, no zone. */
const dateOf = (day: string): Date | undefined => {
  if (!isDay(day)) return undefined;
  const [y = 0, m = 1, d = 1] = day.split("-").map(Number);
  return new Date(y, m - 1, d);
};
const dayOf = (date: Date): string => `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, "0")}-${String(date.getDate()).padStart(2, "0")}`;

/** A calendar day from the kit's Calendar, in a popover: no browser date control. */
export function DayField({ field, label, value, onChange, errors }: { field: string; label: string; value: string; onChange: (day: string) => void; errors: FieldErrors }) {
  const t = useT();
  const locale = useLocale();
  const button = useButtonSize();
  const [open, setOpen] = useState(false);
  const shown = errors.byField.get(field);
  const id = domIdOf(field);
  const selected = dateOf(value);
  const describedBy = describedByOf(field, false, shown?.length ?? 0);
  return (
    <Field className="flex min-w-0 flex-col gap-1" data-invalid={shown ? true : undefined}>
      <FieldLabel htmlFor={id}>{label}</FieldLabel>
      <Popover open={open} onOpenChange={setOpen}>
        <PopoverTrigger asChild>
          <Button id={id} type="button" variant="outline" size={button()} className="w-full justify-start" aria-invalid={shown ? true : undefined}
            {...(describedBy === undefined ? {} : { "aria-describedby": describedBy })}
          >
            <CalendarDays aria-hidden className="size-4" />
            {selected ? formatDay(value, locale) : t("pricing.field.pickDay")}
          </Button>
        </PopoverTrigger>
        <PopoverContent align="start" className="w-auto p-2">
          <Calendar
            {...(selected ? { selected, defaultMonth: selected } : {})}
            locale={locale}
            previousMonthLabel={t("pricing.calendar.previous")}
            nextMonthLabel={t("pricing.calendar.next")}
            onSelect={(date) => {
              onChange(dayOf(date));
              setOpen(false);
            }}
          />
        </PopoverContent>
      </Popover>
      <FieldMessages shown={shown} field={field} />
    </Field>
  );
}
