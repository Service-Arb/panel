"use client";

import { Button, Sidebar, SidebarContent, SidebarFooter, SidebarGroup, SidebarHeader, SidebarMenu, SidebarMenuButton, SidebarMenuItem } from "@evinvest/uikit";
import { ArrowUpRight } from "lucide-react";
import Link from "next/link";
import { usePathname } from "next/navigation";

import { useMe } from "@/entities/session";
import { useT } from "@/shared/i18n";

import { SIDEBAR, isActive, visibleTo } from "../config/nav";
import { useSignOut } from "@/features/sign-out";

export function AppSidebar() {
  const t = useT();
  const me = useMe();
  const pathname = usePathname();
  const signOut = useSignOut();

  return (
    <Sidebar collapsible="none" className="sticky top-0 h-svh w-(--sidebar-width) border-r border-border max-md:hidden">
      <SidebarHeader className="px-4 py-5">
        <span className="text-base font-semibold text-ink">{t("app.name")}</span>
      </SidebarHeader>
      <SidebarContent>
        <SidebarGroup>
          <SidebarMenu>
            {SIDEBAR.filter(visibleTo(me.role)).map((item) => {
              const Icon = item.icon;
              return (
                <SidebarMenuItem key={item.href}>
                  <SidebarMenuButton asChild isActive={!item.external && isActive(pathname, item.href)}>
                    {item.external ? (
                      <a href={item.href} target="_blank" rel="noopener">
                        <Icon aria-hidden />
                        <span>{t(item.key)}</span>
                        <ArrowUpRight aria-hidden className="ml-auto" />
                      </a>
                    ) : (
                      <Link href={item.href}>
                        <Icon aria-hidden />
                        <span>{t(item.key)}</span>
                      </Link>
                    )}
                  </SidebarMenuButton>
                </SidebarMenuItem>
              );
            })}
          </SidebarMenu>
        </SidebarGroup>
      </SidebarContent>
      <SidebarFooter className="gap-1 p-4">
        <span className="truncate text-sm text-ink">{me.preferred_name || me.email}</span>
        <span className="text-xs text-ink-soft">{t(`nav.role.${me.role}`)}</span>
        <Button variant="ghost" size="sm" className="mt-2 self-start px-0" onClick={signOut}>
          {t("nav.signOut")}
        </Button>
      </SidebarFooter>
    </Sidebar>
  );
}
