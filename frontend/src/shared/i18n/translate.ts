import { type MessageKey, en } from "./en";
import { ru } from "./ru";

export type Locale = "en" | "ru";
export type Vars = Record<string, string | number>;
export type T = (key: MessageKey, vars?: Vars) => string;

const CATALOGUES: Record<Locale, Record<MessageKey, string>> = { en, ru };

/** The panel speaks the reader's browser language when it is Russian, English otherwise. */
export function localeOf(language: string | undefined): Locale {
  return language?.toLowerCase().startsWith("ru") ? "ru" : "en";
}

export function translator(locale: Locale): T {
  const messages = CATALOGUES[locale];
  return (key, vars) => {
    const text = messages[key];
    return vars ? text.replace(/\{(\w+)\}/g, (whole, name: string) => (name in vars ? String(vars[name]) : whole)) : text;
  };
}
