"use client";

import { BottomTabBar, type NavItem } from "@evinvest/uikit";
import Link from "next/link";
import { usePathname } from "next/navigation";

import { useT } from "@/shared/i18n";

/** The phone's bar; More stands in for Experiments and Sources, so it stays lit on them. */
export function PanelTabs({ items }: { items: NavItem[] }) {
  const t = useT();
  return <BottomTabBar items={items} pathname={usePathname()} linkComponent={Link} labels={{ nav: t("nav.primary"), badge: (n) => t("nav.badge", { n }) }} />;
}
