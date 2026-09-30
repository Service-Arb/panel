"use client";

import { Button, Field, FieldError, FieldLabel, Input, Select, SelectContent, SelectItem, SelectTrigger, SelectValue, toast } from "@evinvest/uikit";
import { useId, useState } from "react";

import { type Lead, recordPayment, refOf } from "@/entities/lead";
import { CURRENCIES, DEFAULT_CURRENCY } from "@/shared/config/money";
import { useT } from "@/shared/i18n";
import { notifyFailure } from "@/shared/ui/notify";
import { TOUCH_TARGET, useControlSize } from "@/shared/ui/touch";

import { paymentFrom } from "../model/payment";

/** "Payment": the bill, our commission and the currency, in one form (entered by hand in phase 1). */
export function PaymentForm({ lead, onSaved }: { lead: Lead; onSaved: () => void }) {
  const t = useT();
  const id = useId();
  const [billed, setBilled] = useState("");
  const [commission, setCommission] = useState("");
  const [currency, setCurrency] = useState<string>(DEFAULT_CURRENCY);
  const [busy, setBusy] = useState(false);
  const size = useControlSize();
  const payment = paymentFrom(billed, commission, currency);
  const touched = billed !== "" && commission !== "";

  const submit = async () => {
    if (!payment) return;
    setBusy(true);
    try {
      await recordPayment(refOf(lead), payment);
      toast.positive(t("payment.saved"));
      setBilled("");
      setCommission("");
      onSaved();
    } catch (e) {
      notifyFailure(e, t);
    } finally {
      setBusy(false);
    }
  };

  return (
    <form
      className="flex flex-col gap-3"
      onSubmit={(e) => {
        e.preventDefault();
        void submit();
      }}
    >
      <div className="grid grid-cols-(--grid-payment) gap-2">
        <Field className="flex flex-col gap-1">
          <FieldLabel htmlFor={`${id}-billed`}>{t("payment.billed")}</FieldLabel>
          <Input id={`${id}-billed`} size={size} inputMode="decimal" aria-invalid={touched && !payment} value={billed} onChange={(e) => setBilled(e.target.value)} />
        </Field>
        <Field className="flex flex-col gap-1">
          <FieldLabel htmlFor={`${id}-commission`}>{t("payment.commission")}</FieldLabel>
          <Input id={`${id}-commission`} size={size} inputMode="decimal" aria-invalid={touched && !payment} value={commission} onChange={(e) => setCommission(e.target.value)} />
        </Field>
        <Field className="flex flex-col gap-1">
          <FieldLabel htmlFor={`${id}-currency`}>{t("payment.currency")}</FieldLabel>
          <Select value={currency} onValueChange={setCurrency}>
            <SelectTrigger id={`${id}-currency`} size={size} className="w-full">
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
      {touched && !payment && <FieldError>{t("payment.invalid")}</FieldError>}
      <Button type="submit" className={`self-start ${TOUCH_TARGET}`} disabled={busy || !payment}>
        {t("payment.submit")}
      </Button>
    </form>
  );
}
