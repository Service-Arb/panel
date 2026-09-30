import en from "../../../messages/en.json";
import ru from "../../../messages/ru.json";

/** English is the source: its keys are the keys (`messages/en.json`). */
export type MessageKey = keyof typeof en;

/**
 * Every language names every English key — the type checker refuses a missing
 * one; `tests/i18n.test.ts` refuses one English does not have.
 */
export const CATALOGUES = {
  en,
  ru: ru satisfies Record<MessageKey, string>,
} as const;
