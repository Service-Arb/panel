import type { EditedFields, PlaceSettings } from "@/entities/place";

import { type HoursDraftRow, hoursDraftOf, hoursOf, hoursValid } from "./hours-draft";

/** The form's state: text as typed. An empty field means "the site's own". */
export interface SettingsDraft {
  phone: string;
  whatsapp: string;
  hours: HoursDraftRow[];
  serviceArea: string[];
}

export function draftOf(edited: EditedFields): SettingsDraft {
  return { phone: edited.phone ?? "", whatsapp: edited.whatsapp ?? "", hours: hoursDraftOf(edited.hours), serviceArea: [...(edited.serviceArea ?? [])] };
}

/** "+33 6 12-34.56 (78)" → "+33612345678": what people paste, as E.164 has it. */
export function normalisePhone(raw: string): string {
  return raw.replace(/[\s().-]/g, "");
}

const E164 = /^\+[1-9]\d{6,14}$/;

/**
 * A soft check: the server's rule decides (its 422 is shown on the field), this
 * only warns while typing. Empty is fine — the site keeps its own number.
 */
export function looksLikeE164(raw: string): boolean {
  const s = normalisePhone(raw);
  return s === "" || E164.test(s);
}

/** A commune joins the list once, compared without case or surrounding space. */
export function addArea(list: readonly string[], raw: string): string[] {
  const name = raw.trim().replace(/\s+/g, " ");
  if (!name || list.some((n) => n.toLocaleLowerCase() === name.toLocaleLowerCase())) return [...list];
  return [...list, name];
}

export function removeArea(list: readonly string[], name: string): string[] {
  return list.filter((n) => n !== name);
}

export function editedOf(draft: SettingsDraft): EditedFields {
  const out: EditedFields = {};
  const phone = normalisePhone(draft.phone);
  const whatsapp = normalisePhone(draft.whatsapp);
  const hours = hoursOf(draft.hours);
  if (phone) out.phone = phone;
  if (whatsapp) out.whatsapp = whatsapp;
  if (hours) out.hours = hours;
  if (draft.serviceArea.length > 0) out.serviceArea = [...draft.serviceArea];
  return out;
}

/** The settings to PUT: the draft's fields over everything the form does not edit, as it came. */
export function settingsOf(draft: SettingsDraft, base: PlaceSettings): PlaceSettings {
  return { edited: editedOf(draft), rest: base.rest };
}

export function draftValid(draft: SettingsDraft): boolean {
  return hoursValid(draft.hours);
}

export function draftChanged(draft: SettingsDraft, base: PlaceSettings): boolean {
  return JSON.stringify(editedOf(draft)) !== JSON.stringify(normalisedEdited(base.edited));
}

/** The stored fields in the form's own key order, so an untouched form compares equal. */
function normalisedEdited(e: EditedFields): EditedFields {
  return editedOf(draftOf(e));
}
