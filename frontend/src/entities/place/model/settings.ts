import { type Infer, type Parser, arrayOf, bool, nullable, object, oneOf, record, str } from "@/shared/lib/parse";

/** kitstart's `DayOfWeek`: the wire names, Monday first as the site lists them. */
export const DAYS = ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday"] as const;
export type Day = (typeof DAYS)[number];

const hoursRowParser = object({ days: arrayOf(oneOf(DAYS)), opens: str, closes: str });
export type HoursRow = Infer<typeof hoursRowParser>;

/**
 * The fields the panel edits in v1. Everything else a place's live data holds
 * (address, geo, storefrontPhoto, landmark, rating) stays in `rest`, untouched,
 * and goes back as it came: a PUT is a full replace.
 */
export interface EditedFields {
  phone?: string;
  whatsapp?: string;
  hours?: HoursRow[];
  serviceArea?: string[];
}

export const EDITED_KEYS = ["phone", "whatsapp", "hours", "serviceArea"] as const satisfies readonly (keyof EditedFields)[];

export interface PlaceSettings {
  edited: EditedFields;
  rest: Record<string, unknown>;
}

const isEditedKey = (k: string): k is (typeof EDITED_KEYS)[number] => (EDITED_KEYS as readonly string[]).includes(k);

/** The live data as stored, split into what the form edits and what it carries through. */
export const settingsParser: Parser<PlaceSettings> = (v, path) => {
  const o = record(v, path);
  const edited: EditedFields = {};
  if (o.phone !== undefined) edited.phone = str(o.phone, `${path}.phone`);
  if (o.whatsapp !== undefined) edited.whatsapp = str(o.whatsapp, `${path}.whatsapp`);
  if (o.hours !== undefined) edited.hours = arrayOf(hoursRowParser)(o.hours, `${path}.hours`);
  if (o.serviceArea !== undefined) edited.serviceArea = arrayOf(str)(o.serviceArea, `${path}.serviceArea`);
  const rest = Object.fromEntries(Object.entries(o).filter(([k]) => !isEditedKey(k)));
  return { edited, rest };
};

/** The wire object of a PUT: what the form says over what it does not edit. */
export function settingsToWire(s: PlaceSettings): Record<string, unknown> {
  return { ...s.rest, ...s.edited };
}

/** `GET /places/{brand}/{slug}/settings`, and every write's answer. */
export const placeSettingsParser = object({
  brand: str,
  slug: str,
  withdrawn: bool,
  settings: settingsParser,
  updated_at: nullable(str),
  updated_by: nullable(str),
  can_edit: bool,
});
export type PlaceSettingsView = Infer<typeof placeSettingsParser>;

const changeParser = object({ id: str, at: str, by: str, before: settingsParser, after: settingsParser });
export type SettingsChange = Infer<typeof changeParser>;

/** `GET …/settings/history`, newest first. */
export const historyParser = object({ changes: arrayOf(changeParser) });
