"use client";

import { Button, Field, FieldLabel, Input, Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@evinvest/uikit";
import { useId, useState } from "react";

import type { StageMove } from "@/entities/lead";
import { CURRENCIES, DEFAULT_CURRENCY } from "@/shared/config/money";
import { useT } from "@/shared/i18n";
import { parseMoney } from "@/shared/lib/format";

/** A quote with an optional amount: both amount and currency, or neither. */
export function QuoteForm({ busy, onSubmit, onCancel }: { busy: boolean; onSubmit: (m: StageMove) => void; onCancel: () => void }) {
  const t = useT();
  const id = useId();
  const [amount, setAmount] = useState("");
  const [currency, setCurrency] = useState<string>(DEFAULT_CURRENCY);
  const minor = amount.trim() === "" ? null : parseMoney(amount);
  const invalid = amount.trim() !== "" && minor === null;

  return (
    <form
      className="flex flex-col gap-3"
      onSubmit={(e) => {
        e.preventDefault();
        if (invalid) return;
        onSubmit(minor === null ? { stage: "quoted" } : { stage: "quoted", amount: minor, currency });
      }}
    >
      <div className="flex gap-2">
        <Field className="flex flex-1 flex-col gap-1">
          <FieldLabel htmlFor={`${id}-amount`}>{t("move.amount")}</FieldLabel>
          <Input id={`${id}-amount`} inputMode="decimal" value={amount} aria-invalid={invalid} onChange={(e) => setAmount(e.target.value)} />
        </Field>
        <Field className="flex w-28 flex-col gap-1">
          <FieldLabel>{t("move.currency")}</FieldLabel>
          <Select value={currency} onValueChange={setCurrency}>
            <SelectTrigger aria-label={t("move.currency")}>
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {CURRENCIES.map((c) => (
                <SelectItem key={c} value={c}>
                  {c}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </Field>
      </div>
      <FormButtons busy={busy || invalid} onCancel={onCancel} />
    </form>
  );
}

export function FormButtons({ busy, onCancel, submitLabel }: { busy: boolean; onCancel: () => void; submitLabel?: string }) {
  const t = useT();
  return (
    <div className="flex gap-2">
      <Button type="submit" disabled={busy}>
        {submitLabel ?? t("move.submit")}
      </Button>
      <Button type="button" variant="ghost" onClick={onCancel}>
        {t("move.cancel")}
      </Button>
    </div>
  );
}
