import { PAGE_PROVIDERS } from "@/shared/config/booking";
import type { T } from "@/shared/i18n";

import type { BookingConfig } from "../model/booking";

/** "Default: Google Calendar; Link: https://…; Google Calendar: https://…" — the history's line, in form order. */
export function bookingText(b: BookingConfig, t: T): string {
  const pages = PAGE_PROVIDERS.flatMap((p) => {
    const url = b.providers[p]?.url;
    return url === undefined ? [] : [`${t(`booking.provider.${p}`)}: ${url}`];
  });
  return [t("placeSettings.booking.defaultIs", { provider: t(`booking.provider.${b.default}`) }), ...pages].join("; ");
}
