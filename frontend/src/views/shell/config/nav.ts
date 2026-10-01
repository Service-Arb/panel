import { BarChart3, Ellipsis, FlaskConical, Inbox, KeyRound, LayoutGrid, MapPin, type LucideIcon } from "lucide-react";

import type { Role } from "@/entities/session";
import { ROUTES } from "@/shared/config/routes";
import type { MessageKey } from "@/shared/i18n";

export interface NavItem {
  href: string;
  key: MessageKey;
  icon: LucideIcon;
  /** Leaves the app: Grafana is a separate instance behind the backend (spec §6). */
  external?: true;
  roles?: readonly Role[];
  /** Other screens this entry stands for, lit with it: "More" holds Sources on a phone. */
  also?: readonly string[];
}

/** The desktop sidebar (spec §10: hybrid 1+2). */
export const SIDEBAR: readonly NavItem[] = [
  { href: ROUTES.overview, key: "nav.overview", icon: BarChart3 },
  { href: ROUTES.leads, key: "nav.leads", icon: Inbox },
  { href: ROUTES.places, key: "nav.places", icon: MapPin },
  { href: ROUTES.experiments, key: "nav.experiments", icon: FlaskConical },
  { href: ROUTES.grafana, key: "nav.grafana", icon: LayoutGrid, external: true },
  { href: ROUTES.sources, key: "nav.sources", icon: KeyRound, roles: ["admin"] },
];

/** The phone's bottom bar: the three screens, and "More" for the rest. */
export const TABS: readonly NavItem[] = [
  { href: ROUTES.overview, key: "nav.overview", icon: BarChart3 },
  { href: ROUTES.leads, key: "nav.leads", icon: Inbox },
  { href: ROUTES.places, key: "nav.places", icon: MapPin },
  { href: ROUTES.more, key: "nav.more", icon: Ellipsis, also: [ROUTES.experiments, ROUTES.sources] },
];

export function visibleTo(role: Role) {
  return (item: NavItem) => !item.roles || item.roles.includes(role);
}

/** `/leads/` and `/leads` are the same screen: the export writes one, the dev server serves the other. */
export function isActive(pathname: string, href: string, also: readonly string[] = []): boolean {
  const strip = (p: string) => p.replace(/\/+$/, "") || "/";
  return [href, ...also].some((h) => strip(pathname) === strip(h));
}
