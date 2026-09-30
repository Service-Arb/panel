"use client";

import { SidebarInset, SidebarProvider, Toaster } from "@evinvest/uikit";
import type { ReactNode } from "react";

import { MeProvider, fetchMe } from "@/entities/session";
import { useResource } from "@/shared/lib/use-resource";
import { ErrorState } from "@/shared/ui/error-state";
import { DESKTOP_QUERY, useMediaQuery } from "@/shared/lib/use-media-query";

import { NoAccessScreen, ShellSkeleton, UnavailableScreen } from "./access-screens";
import { AppSidebar } from "./app-sidebar";
import { TabBar } from "./tab-bar";

/**
 * Everything behind sign-in. `/api/v1/me` decides what shows: a 401 has already
 * sent the browser to `/auth/login` (the skeleton stays up meanwhile), a 403 is
 * "no access", a 503 is "try again" without signing anyone out.
 */
export function PanelShell({ children }: { children: ReactNode }) {
  const me = useResource("me", fetchMe);
  const isDesktop = useMediaQuery(DESKTOP_QUERY);

  if (me.status === "loading") return <ShellSkeleton />;
  if (me.status === "error") {
    switch (me.failure.kind) {
      case "unauthenticated":
        return <ShellSkeleton />;
      case "no_access":
        return <NoAccessScreen />;
      case "unavailable":
        return <UnavailableScreen onRetry={me.reload} />;
      default:
        return (
          <div className="p-6">
            <ErrorState failure={me.failure} onRetry={me.reload} />
          </div>
        );
    }
  }

  return (
    <MeProvider me={me.data}>
      <SidebarProvider>
        <AppSidebar />
        <SidebarInset className="min-w-0 pb-[calc(var(--panel-tabbar-h)+env(safe-area-inset-bottom,0px))] md:pb-0">{children}</SidebarInset>
        <TabBar />
      </SidebarProvider>
      {/* On a phone the bottom is the tab bar and the sheets; toasts come from the top. */}
      <Toaster position={isDesktop ? "bottom-right" : "top-center"} />
    </MeProvider>
  );
}
