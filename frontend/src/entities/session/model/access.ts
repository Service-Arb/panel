import { ROUTES } from "@/shared/config/routes";

import type { Caller, Permission } from "./generated";

/** Whether the caller holds `permission`: the set concierge resolved is concrete. */
export function may(caller: Caller, permission: Permission): boolean {
  return caller.permissions.includes(permission);
}

/**
 * Where a person lands (spec §10): whoever manages the sources reads the funnel first;
 * everyone else's job is the queue of new leads.
 */
export function startRouteFor(caller: Caller): string {
  return may(caller, "sa:admin:sources:manage") ? ROUTES.overview : `${ROUTES.leads}?stage=created`;
}
