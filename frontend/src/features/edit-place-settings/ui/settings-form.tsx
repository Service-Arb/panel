"use client";

import { Alert, AlertDescription, AlertTitle, Button } from "@evinvest/uikit";
import { useState } from "react";

import { ChannelPreview, type PlaceSettingsView } from "@/entities/place";
import { useT } from "@/shared/i18n";
import { useButtonSize } from "@/shared/ui/touch";

import { type SettingsDraft, draftChanged, draftOf, draftValid, editedOf, settingsOf } from "../model/draft";
import { useSave } from "../model/use-save";
import { AreaChips } from "./area-chips";
import { HoursEditor } from "./hours-editor";
import { KeptFields } from "./kept-fields";
import { PhoneField } from "./phone-field";

export interface SettingsFormProps {
  place: PlaceSettingsView;
  onSaved: (view: PlaceSettingsView) => void;
  /** After a 409: read the place again, dropping this form's edits. */
  onReload: () => void;
}

/**
 * The editor of a place's live data. Re-mount it (key on `updated_at`) when the
 * place changes, so the draft starts from what was saved.
 */
export function SettingsForm({ place, onSaved, onReload }: SettingsFormProps) {
  const t = useT();
  const button = useButtonSize();
  const base = place.settings;
  const [draft, setDraft] = useState<SettingsDraft>(() => draftOf(base.edited));
  const { state, errors, save } = useSave(place, place.updated_at, onSaved);
  const set = (patch: Partial<SettingsDraft>) => setDraft((d) => ({ ...d, ...patch }));
  const changed = draftChanged(draft, base);

  return (
    <form
      className="flex flex-col gap-4"
      onSubmit={(e) => {
        e.preventDefault();
        if (changed && draftValid(draft)) void save(settingsOf(draft, base));
      }}
    >
      {state.kind === "conflict" && (
        <Alert variant="destructive" className="flex flex-col gap-2">
          <AlertTitle>{t("placeSettings.conflict.title")}</AlertTitle>
          <AlertDescription>{t("placeSettings.conflict.body")}</AlertDescription>
          <Button type="button" variant="outline" size={button("sm")} className="self-start" onClick={onReload}>
            {t("placeSettings.conflict.reload")}
          </Button>
        </Alert>
      )}
      {state.kind === "invalid" && (
        <Alert variant="destructive">
          <AlertDescription>
            {t("placeSettings.invalid")}
            {errors.other.map((o) => (
              <span key={o.key} className="block">{`${o.key}: ${o.reason}`}</span>
            ))}
          </AlertDescription>
        </Alert>
      )}
      <PhoneField label={t("placeSettings.field.phone")} value={draft.phone} onChange={(phone) => set({ phone })} errors={errors.byField.phone} />
      <PhoneField label={t("placeSettings.field.whatsapp")} value={draft.whatsapp} onChange={(whatsapp) => set({ whatsapp })} errors={errors.byField.whatsapp} />
      <HoursEditor rows={draft.hours} onChange={(hours) => set({ hours })} errors={errors.byField.hours} />
      <AreaChips names={draft.serviceArea} onChange={(serviceArea) => set({ serviceArea })} errors={errors.byField.serviceArea} />
      <KeptFields rest={base.rest} />
      <ChannelPreview fields={editedOf(draft)} />
      <div className="flex flex-wrap gap-2">
        <Button type="submit" size={button()} disabled={!changed || !draftValid(draft) || state.kind === "saving" || state.kind === "conflict"}>
          {t("placeSettings.save")}
        </Button>
        <Button type="button" variant="ghost" size={button()} disabled={!changed || state.kind === "saving"} onClick={() => setDraft(draftOf(base.edited))}>
          {t("placeSettings.reset")}
        </Button>
      </div>
    </form>
  );
}
