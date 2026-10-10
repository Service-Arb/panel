"use client";

import { Field, FieldDescription, FieldError, FieldLabel, Input } from "@evinvest/uikit";
import { useId } from "react";

import { useT } from "@/shared/i18n";
import { useControlSize } from "@/shared/ui/touch";

import { looksLikeBrandName } from "../model/draft";

/** The brand name for customer text: a soft hint while typing, the server's own reason once it refuses. */
export function BrandNameField({ value, onChange, errors }: { value: string; onChange: (v: string) => void; errors: string[] | undefined }) {
  const t = useT();
  const id = useId();
  const size = useControlSize();
  return (
    <Field className="flex flex-col gap-1" data-invalid={errors ? true : undefined}>
      <FieldLabel htmlFor={id}>{t("placeSettings.field.brandName")}</FieldLabel>
      <Input id={id} type="text" autoComplete="off" size={size} value={value} aria-invalid={errors ? true : undefined} onChange={(e) => onChange(e.target.value)} />
      {looksLikeBrandName(value) ? <FieldDescription>{t("placeSettings.brandName.hint")}</FieldDescription> : <FieldDescription className="text-accent-warn">{t("placeSettings.brandName.invalid")}</FieldDescription>}
      {errors?.map((reason) => <FieldError key={reason}>{reason}</FieldError>)}
    </Field>
  );
}
