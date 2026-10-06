import { ROUTES } from "@/shared/config/routes";

import type { Role } from "./generated";

/**
 * Where a person lands (spec §10): an operator's job is the queue of new leads,
 * so their start is "Leads" filtered to new ones; an admin reads the funnel first.
 */
export function startRouteFor(role: Role): string {
  return role === "operator" ? `${ROUTES.leads}?stage=created` : ROUTES.overview;
}
