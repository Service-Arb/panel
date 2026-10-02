import { EDITED_KEYS } from "@/entities/place";

type FieldKey = (typeof EDITED_KEYS)[number];

export interface FieldErrors {
  /** Per form field: every reason whose key is the field or a path inside it (`hours[0].opens`). */
  byField: Partial<Record<FieldKey, string[]>>;
  /** Reasons for keys the form has no field for, shown above it rather than lost. */
  other: { key: string; reason: string }[];
}

const owner = (key: string): FieldKey | null => EDITED_KEYS.find((f) => key === f || key.startsWith(`${f}.`) || key.startsWith(`${f}[`)) ?? null;

/** A 422's `fields` sorted onto the form. */
export function fieldErrorsOf(fields: Record<string, string>): FieldErrors {
  const out: FieldErrors = { byField: {}, other: [] };
  for (const [key, reason] of Object.entries(fields)) {
    const f = owner(key);
    if (f) out.byField[f] = [...(out.byField[f] ?? []), reason];
    else out.other.push({ key, reason });
  }
  return out;
}

export const NO_ERRORS: FieldErrors = { byField: {}, other: [] };
