"use client";

import { Button, type NavGroup, ShellNav } from "@evinvest/uikit";
import Link from "next/link";
import { usePathname, useRouter } from "next/navigation";

import { DevSignInBadge, useMe } from "@/entities/session";
import { useSignOut } from "@/features/sign-out";
import { useT } from "@/shared/i18n";
import { LiveStatusIndicator } from "@/shared/ui/live-status";

/** The desktop rail: the app's name (and the dev sign-in warning), the groups, then who is signed in. */
export function PanelRail({ groups }: { groups: NavGroup[] }) {
  const t = useT();
  const router = useRouter();
  return (
    <ShellNav
      groups={groups}
      pathname={usePathname()}
      linkComponent={Link}
      onItemIntent={(item) => {
        if (!item.external) router.prefetch(item.href);
      }}
      header={
        <div className="flex flex-col items-start gap-2 px-3">
          <span className="text-base font-semibold text-ink">{t("app.name")}</span>
          <DevSignInBadge />
        </div>
      }
      footer={<AccountBlock />}
      labels={{ primary: t("nav.primary"), badge: (n) => t("nav.badge", { n }) }}
    />
  );
}

function AccountBlock() {
  const t = useT();
  const me = useMe();
  const signOut = useSignOut();
  return (
    <div className="flex flex-col gap-1 border-t border-border px-3 pt-4">
      <LiveStatusIndicator className="mb-2" />
      <span className="truncate text-sm text-ink" title={me.email}>
        {me.preferred_name || me.email}
      </span>
      <span className="text-xs text-ink-soft">{t(`nav.role.${me.role}`)}</span>
      <Button variant="ghost" size="sm" className="mt-2 self-start px-0" onClick={signOut}>
        {t("nav.signOut")}
      </Button>
    </div>
  );
}
