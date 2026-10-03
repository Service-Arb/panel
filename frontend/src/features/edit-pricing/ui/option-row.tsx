"use client";

import { Button } from "@evinvest/uikit";
import { Trash2 } from "lucide-react";

import { useT } from "@/shared/i18n";
import { useButtonSize } from "@/shared/ui/touch";

import type { InputDraft, OptionDraft } from "../model/draft";
import { FIELD, domIdOf } from "../model/fields";
import { removeOption, updateOption } from "../model/ops";
import type { PricingEditor } from "../model/use-pricing-editor";
import { FieldMessages } from "./field-messages";
import { LabelsFields } from "./labels-fields";
import { TextField } from "./text-field";

/** One answer to a question: its slug, what it does to the price, and its words. */
export function OptionRow({ editor, input, option, first }: { editor: PricingEditor; input: InputDraft; option: OptionDraft; first: boolean }) {
  const t = useT();
  const button = useButtonSize();
  const set = (patch: Partial<Omit<OptionDraft, "key">>) => editor.update((d) => updateOption(d, input.key, option.key, patch));
  return (
    <li id={domIdOf(FIELD.option(option.key))} tabIndex={-1} className="flex items-start gap-2 border-b border-border pb-3 outline-none last:border-b-0">
      {/* One row from lg up: slug and effect, then the labels; a phone stacks them. */}
      <div className="grid min-w-0 flex-1 items-start gap-2 lg:grid-cols-2">
        <div className="grid items-start gap-2 sm:grid-cols-2">
          <TextField field={FIELD.optionId(option.key)} label={t("pricing.field.optionId")} kind="slug" value={option.id} onChange={(id) => set({ id })} errors={editor.errors} />
          <TextField
            field={FIELD.optionValue(option.key)}
            label={t(`pricing.field.effect.${input.kind}`)}
            {...(first ? { hint: t(`pricing.hint.effect.${input.kind}`) } : {})}
            kind="amount"
            value={option.value}
            onChange={(value) => set({ value })}
            errors={editor.errors}
          />
        </div>
        <LabelsFields labels={option.labels} locales={editor.base.locales} field={FIELD.optionLabels(option.key)} onChange={(labels) => set({ labels })} errors={editor.errors} />
        <FieldMessages shown={editor.errors.byField.get(FIELD.option(option.key))} />
      </div>
      <Button
        type="button"
        variant="ghost"
        size={button("sm")}
        icon
        className="mt-6 shrink-0"
        aria-label={t("pricing.option.remove")}
        disabled={input.options.length <= 1}
        onClick={() => editor.update((d) => removeOption(d, input.key, option.key))}
      >
        <Trash2 aria-hidden className="size-4" />
      </Button>
    </li>
  );
}
