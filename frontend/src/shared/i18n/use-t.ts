"use client";

import { useEffect, useMemo, useSyncExternalStore } from "react";

import { type Locale, type T, localeOf, translator } from "./translate";

const subscribe = (onChange: () => void) => {
  window.addEventListener("languagechange", onChange);
  return () => window.removeEventListener("languagechange", onChange);
};

/** The export is prerendered in English; the browser's language takes over on hydration. */
export function useLocale(): Locale {
  return useSyncExternalStore(
    subscribe,
    () => localeOf(navigator.language),
    () => "en",
  );
}

/** Keeps `<html lang>` on the language the page is actually in (the export says "en"). */
export function DocumentLang() {
  const locale = useLocale();
  useEffect(() => {
    document.documentElement.lang = locale;
  }, [locale]);
  return null;
}

export function useT(): T {
  const locale = useLocale();
  return useMemo(() => translator(locale), [locale]);
}
