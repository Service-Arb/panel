import { ROUTES } from "@/shared/config/routes";

/** The roles the panel admits (spec §5.4); viewer is gone (§11). */
export const ROLES = ["operator", "admin"] as const;
export type Role = (typeof ROLES)[number];

export interface Me {
  user_id: string;
  role: Role;
  email: string;
  preferred_name: string;
}

/**
 * Where a person lands (spec §10): an operator's job is the queue of new leads,
 * so their start is "Leads" filtered to new ones; an admin reads the funnel first.
 */
export function startRouteFor(role: Role): string {
  return role === "operator" ? `${ROUTES.leads}?stage=created` : ROUTES.overview;
}

export function managesSources(role: Role): boolean {
  return role === "admin";
}
