/**
 * Names of the editor's fields, by draft key: what an error is filed under and
 * what a "show me" link focuses. `input:k3:labels:fr` sits inside
 * `input:k3:labels`, which sits inside `input:k3`.
 */
export const FIELD = {
  validFrom: "validFrom",
  roundTo: "roundTo",
  minimum: "minimum",
  inputs: "inputs",
  needs: "needs",
  input: (k: string) => `input:${k}`,
  inputId: (k: string) => `input:${k}:id`,
  inputKind: (k: string) => `input:${k}:kind`,
  inputLabels: (k: string) => `input:${k}:labels`,
  inputLabel: (k: string, locale: string) => `input:${k}:labels:${locale}`,
  inputOptions: (k: string) => `input:${k}:options`,
  option: (k: string) => `option:${k}`,
  optionId: (k: string) => `option:${k}:id`,
  optionLabels: (k: string) => `option:${k}:labels`,
  optionLabel: (k: string, locale: string) => `option:${k}:labels:${locale}`,
  optionValue: (k: string) => `option:${k}:value`,
  need: (k: string) => `need:${k}`,
  needId: (k: string) => `need:${k}:id`,
  needKind: (k: string) => `need:${k}:kind`,
  needAmount: (k: string) => `need:${k}:amount`,
  needInputs: (k: string) => `need:${k}:inputs`,
} as const;

/** `field` is `parent` or inside it. */
export const within = (field: string, parent: string): boolean => field === parent || field.startsWith(`${parent}:`);

/** The DOM id of a field's control (or of its group): what a link to the field focuses. */
export const domIdOf = (field: string): string => `pricing-${field.replace(/[^a-zA-Z0-9_-]/g, "-")}`;
