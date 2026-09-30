"use client";

import { DESKTOP_QUERY, useMediaQuery } from "@/shared/lib/use-media-query";

/**
 * A 44px minimum for a Button on a phone (WCAG 2.5.5 / Apple HIG); the kit's
 * Button sizes stop at 40px.
 * TODO(uikit): replace with the kit's touch size once lib ships one — one
 * constant, so the swap is one line.
 */
export const TOUCH_TARGET = "max-md:min-h-11";

/** Input and SelectTrigger: the kit's `lg` (48px) on a phone, `md` from `md` up. */
export function useControlSize(): "md" | "lg" {
  return useMediaQuery(DESKTOP_QUERY) ? "md" : "lg";
}
