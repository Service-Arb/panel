"use client";

import { Button, Field, FieldDescription, FieldLabel, Input, Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@evinvest/uikit";
import { useId, useState } from "react";

import { type AddedSource, SOURCE_KINDS, type SourceKind, addSource } from "@/entities/source";
import { isSlug } from "@/shared/config/brands";
import { useT } from "@/shared/i18n";
import { notifyFailure } from "@/shared/ui/notify";

import { brandsFrom } from "../model/brands";

/** A new signing key: its id, the one kind it writes as, and the brands it may write for. */
export function AddSourceForm({ onAdded }: { onAdded: (added: AddedSource) => void }) {
  const t = useT();
  const id = useId();
  const [keyId, setKeyId] = useState("");
  const [kind, setKind] = useState<SourceKind>("site");
  const [brandsText, setBrandsText] = useState("");
  const [busy, setBusy] = useState(false);
  const brands = brandsFrom(brandsText);
  const ready = isSlug(keyId) && brands !== null;

  const submit = async () => {
    if (!brands) return;
    setBusy(true);
    try {
      onAdded(await addSource({ key_id: keyId, kind, brands }));
      setKeyId("");
      setBrandsText("");
    } catch (e) {
      notifyFailure(e, t);
    } finally {
      setBusy(false);
    }
  };

  return (
    <form
      className="grid gap-3 md:grid-cols-(--grid-source-form) md:items-end"
      onSubmit={(e) => {
        e.preventDefault();
        if (ready) void submit();
      }}
    >
      <Field className="flex flex-col gap-1">
        <FieldLabel htmlFor={`${id}-key`}>{t("sources.keyId")}</FieldLabel>
        <Input id={`${id}-key`} autoCapitalize="none" value={keyId} onChange={(e) => setKeyId(e.target.value.toLowerCase())} />
      </Field>
      <Field className="flex flex-col gap-1">
        <FieldLabel>{t("sources.kind")}</FieldLabel>
        <Select value={kind} onValueChange={(v) => setKind(SOURCE_KINDS.find((k) => k === v) ?? "site")}>
          <SelectTrigger aria-label={t("sources.kind")}>
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            {SOURCE_KINDS.map((k) => (
              <SelectItem key={k} value={k}>
                {k}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      </Field>
      <Field className="flex flex-col gap-1">
        <FieldLabel htmlFor={`${id}-brands`}>{t("sources.brands")}</FieldLabel>
        <Input id={`${id}-brands`} autoCapitalize="none" value={brandsText} onChange={(e) => setBrandsText(e.target.value)} />
        <FieldDescription>{t("sources.brands.hint")}</FieldDescription>
      </Field>
      <Button type="submit" disabled={busy || !ready} className="md:mb-6">
        {t("sources.add")}
      </Button>
    </form>
  );
}
