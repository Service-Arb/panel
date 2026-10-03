import { NavDot, type NavGroup, type NavItem } from "@evinvest/uikit";
import { ArrowUpRight, BarChart3, Ellipsis, FlaskConical, Inbox, KeyRound, LayoutGrid, MapPin } from "lucide-react";

import { type Role, managesSources } from "@/entities/session";
import { ROUTES } from "@/shared/config/routes";
import type { T } from "@/shared/i18n";

import type { Marks } from "../model/use-marks";

export interface PanelNav {
  groups: NavGroup[];
  footer: NavGroup[];
  tabs: NavItem[];
}

/**
 * The rail (spec §10, hybrid 1+2) in groups, and the phone's four tabs: the
 * three daily screens, and More standing in for the rest. Marks ride on both:
 * a count where there is one, a dot where "something changed" is all there is.
 */
export function panelNav(t: T, role: Role, marks: Marks): PanelNav {
  const admin = managesSources(role);
  const dot = (corner: boolean) => <NavDot corner={corner} label={t("nav.changed")} />;
  const overview: NavItem = { id: "overview", href: ROUTES.overview, label: t("nav.overview"), icon: BarChart3 };
  const leads: NavItem = { id: "leads", href: ROUTES.leads, label: t("nav.leads"), icon: Inbox, badge: marks.leads };
  const places: NavItem = { id: "places", href: ROUTES.places, label: t("nav.places"), icon: MapPin, badge: marks.places };
  const experiments: NavItem = { id: "experiments", href: ROUTES.experiments, label: t("nav.experiments"), icon: FlaskConical, ...(marks.experiments ? { badge: dot(false) } : {}) };
  const sources: NavItem = { id: "sources", href: ROUTES.sources, label: t("nav.sources"), icon: KeyRound, ...(marks.sources ? { badge: dot(false) } : {}) };
  const grafana: NavItem = {
    id: "grafana",
    href: ROUTES.grafana,
    label: t("nav.grafana"),
    icon: LayoutGrid,
    external: true,
    target: "_blank",
    trailing: <ArrowUpRight aria-hidden className="size-4 text-ink-soft" />,
  };
  const elsewhere = marks.experiments || (admin && marks.sources);
  const more: NavItem = {
    id: "more",
    href: ROUTES.more,
    label: t("nav.more"),
    icon: Ellipsis,
    also: admin ? [ROUTES.experiments, ROUTES.sources] : [ROUTES.experiments],
    ...(elsewhere ? { badge: dot(true) } : {}),
  };

  const groups: NavGroup[] = [
    { id: "work", label: t("nav.group.work"), items: [overview, leads, places] },
    { id: "analysis", label: t("nav.group.analysis"), items: [experiments] },
  ];
  if (admin) groups.push({ id: "admin", label: t("nav.group.admin"), items: [sources] });
  return {
    groups,
    footer: [{ id: "account", label: t("nav.group.account"), items: [grafana] }],
    tabs: [overview, leads, places, more],
  };
}
