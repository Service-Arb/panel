"use client";

import { Field, FieldDescription, FieldError, FieldLabel, Input } from "@evinvest/uikit";
import { useId } from "react";

import { useT } from "@/shared/i18n";
import { useControlSize } from "@/shared/ui/touch";

import { looksLikeReviewUrl } from "../model/draft";

/** The Google review link: a soft hint while typing, the server's own reason once it refuses. */
export function ReviewUrlField({ value, onChange, errors }: { value: string; onChange: (v: string) => void; errors: string[] | undefined }) {
  const t = useT();
  const id = useId();
  const size = useControlSize();
  return (
    <Field className="flex flex-col gap-1" data-invalid={errors ? true : undefined}>
      <FieldLabel htmlFor={id}>{t("placeSettings.field.reviewUrl")}</FieldLabel>
      <Input id={id} type="url" inputMode="url" autoCapitalize="none" autoComplete="off" spellCheck={false} size={size} value={value} placeholder="https://g.page/r/…/review" aria-invalid={errors ? true : undefined} onChange={(e) => onChange(e.target.value)} />
      {looksLikeReviewUrl(value) ? <FieldDescription>{t("placeSettings.reviewUrl.hint")}</FieldDescription> : <FieldDescription className="text-accent-warn">{t("placeSettings.reviewUrl.invalid")}</FieldDescription>}
      {errors?.map((reason) => <FieldError key={reason}>{reason}</FieldError>)}
    </Field>
  );
}
