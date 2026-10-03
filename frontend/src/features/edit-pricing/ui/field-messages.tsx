"use client";

import { FieldError } from "@evinvest/uikit";

import { useT } from "@/shared/i18n";

import type { Shown } from "../model/errors";
import { shownText } from "./shown-text";

/** The reasons filed under one field, each on its own line. */
export function FieldMessages({ shown }: { shown: readonly Shown[] | undefined }) {
  const t = useT();
  if (!shown || shown.length === 0) return null;
  return (
    <>
      {shown.map((s, i) => (
        <FieldError key={i}>{shownText(s, t)}</FieldError>
      ))}
    </>
  );
}
