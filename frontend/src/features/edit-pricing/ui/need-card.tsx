"use client";

import { Button, Card, CardAction, CardContent, CardHeader, CardTitle } from "@evinvest/uikit";
import { Trash2 } from "lucide-react";

import { NEED_KINDS, type NeedKind } from "@/entities/pricing";
import { useT } from "@/shared/i18n";
import { useButtonSize } from "@/shared/ui/touch";

import type { NeedDraft } from "../model/draft";
import { FIELD, domIdOf } from "../model/fields";
import { removeNeed, updateNeed } from "../model/ops";
import type { PricingEditor } from "../model/use-pricing-editor";
import { AskedInputs } from "./asked-inputs";
import { FieldMessages } from "./field-messages";
import { KindSelect } from "./kind-select";
import { TextField } from "./text-field";

/** How one need (the site's subject slug) is priced: an estimate from a base and answers, or one fixed price. */
export function NeedCard({ editor, need }: { editor: PricingEditor; need: NeedDraft }) {
  const t = useT();
  const button = useButtonSize();
  const { errors } = editor;
  const title = need.id || t("pricing.need.untitled");
  const set = (patch: Partial<Omit<NeedDraft, "key">>) => editor.update((d) => updateNeed(d, need.key, patch));
  return (
    <Card id={domIdOf(FIELD.need(need.key))} tabIndex={-1} className="outline-none">
      <CardHeader>
        <CardTitle className="font-mono wrap-anywhere">{title}</CardTitle>
        <CardAction>
          <Button
            type="button"
            variant="ghost"
            size={button("sm")}
            aria-label={t("pricing.field.named", { name: title, field: t("pricing.need.remove") })}
            onClick={() => editor.update((d) => removeNeed(d, need.key))}
          >
            <Trash2 aria-hidden className="size-4" />
            {t("pricing.need.remove")}
          </Button>
        </CardAction>
      </CardHeader>
      <CardContent className="flex flex-col gap-3">
        <FieldMessages shown={errors.byField.get(FIELD.need(need.key))} />
        <div className="grid items-start gap-2 sm:grid-cols-3">
          <TextField field={FIELD.needId(need.key)} label={t("pricing.field.needId")} context={title} hint={t("pricing.hint.need")} kind="slug" value={need.id} onChange={(id) => set({ id })} errors={errors} />
          <KindSelect<NeedKind>
            field={FIELD.needKind(need.key)}
            label={t("pricing.field.needKind")}
            context={title}
            value={need.kind}
            options={NEED_KINDS.map((k) => ({ value: k, label: t(`pricing.needKind.${k}`) }))}
            onChange={(kind) => set({ kind })}
            errors={errors}
          />
          <TextField
            field={FIELD.needAmount(need.key)}
            label={t(need.kind === "fixed" ? "pricing.field.fixedPrice" : "pricing.field.base")}
            context={title}
            kind="amount"
            value={need.amount}
            onChange={(amount) => set({ amount })}
            errors={errors}
          />
        </div>
        {need.kind === "estimate" ? <AskedInputs editor={editor} need={need} /> : <p className="text-sm text-ink-soft">{t("pricing.need.fixedNote")}</p>}
      </CardContent>
    </Card>
  );
}
