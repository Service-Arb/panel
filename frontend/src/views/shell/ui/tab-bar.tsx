"use client";

import { cn } from "@evinvest/uikit";
import Link from "next/link";
import { usePathname } from "next/navigation";

import { useT } from "@/shared/i18n";

import { TABS, isActive } from "../config/nav";

/**
 * The phone's navigation: fixed to the bottom, above the home indicator
 * (`safe-area-inset-bottom`), hidden from `md` up where the sidebar shows.
 */
export function TabBar() {
  const t = useT();
  const pathname = usePathname();
  return (
    <nav
      aria-label={t("nav.primary")}
      className="fixed inset-x-0 bottom-0 z-40 flex h-[calc(var(--panel-tabbar-h)+env(safe-area-inset-bottom,0px))] border-t border-border bg-secondary px-2 pb-[env(safe-area-inset-bottom,0px)] md:hidden"
    >
      {TABS.map((tab) => {
        const Icon = tab.icon;
        const active = isActive(pathname, tab.href, tab.also);
        return (
          <Link
            key={tab.href}
            href={tab.href}
            aria-current={active ? "page" : undefined}
            className={cn(
              "flex min-w-0 flex-1 flex-col items-center justify-center gap-0.5 rounded-lg text-xs font-medium outline-none focus-visible:ring-2 focus-visible:ring-ring",
              active ? "text-primary-ink" : "text-ink-soft hover:text-ink",
            )}
          >
            <Icon aria-hidden className="size-5" />
            <span className="w-full truncate text-center">{t(tab.key)}</span>
          </Link>
        );
      })}
    </nav>
  );
}
