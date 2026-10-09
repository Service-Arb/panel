"use client";

import {
  AccountMenu,
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
  TopBar,
  cn,
} from "@evinvest/uikit";
import { Check, ChevronDown } from "lucide-react";
import Link from "next/link";
import { usePathname } from "next/navigation";

import { may, useMe } from "@/entities/session";
import { useTokens } from "@/entities/tokens";
import { useSignOut } from "@/features/sign-out";
import { switchAccountPath } from "@/shared/api";
import { ROUTES } from "@/shared/config/routes";
import { useT } from "@/shared/i18n";
import { LiveStatusIndicator } from "@/shared/ui/live-status";

/** The wide screen's top bar (the kit's contract): where you are, your tokens, the connection, who you are. */
export function PanelTopBar() {
  const t = useT();
  const me = useMe();
  const signOut = useSignOut();
  const pathname = usePathname();
  return (
    <TopBar
      start={<AppSwitcher accountCenter={me.account_center} />}
      end={
        <>
          <TokensStat />
          <LiveStatusIndicator />
          <AccountMenu
            account={{ name: me.preferred_name, email: me.email }}
            manageHref={me.account_center ?? undefined}
            switchHref={switchAccountPath(pathname)}
            groups={[
              {
                id: "sa",
                items: [
                  { id: "account", href: ROUTES.account, label: t("account.title") },
                  ...(may(me, "sa:review_archive:members:act_as")
                    ? [{ id: "act-as", href: ROUTES.reviewArchiveActAs, label: t("account.actAs"), external: true }]
                    : []),
                  { id: "tokens", href: ROUTES.reviewArchiveTokens, label: t("account.tokens.history"), external: true },
                ],
              },
            ]}
            onSignOut={signOut}
            linkComponent={Link}
            labels={{ trigger: t("nav.account"), manage: t("account.manage"), switchAccount: t("nav.switchAccount"), signOut: t("nav.signOut") }}
          />
        </>
      }
    />
  );
}

/** Service-Arb and evinvest.ltd; just the name when there is no evinvest.ltd to go to (dev sign-in). */
function AppSwitcher({ accountCenter }: { accountCenter: string | null }) {
  const t = useT();
  const name = <span className="text-base font-semibold text-ink">{t("app.name")}</span>;
  if (accountCenter === null) return name;
  return (
    <DropdownMenu>
      <DropdownMenuTrigger className="flex items-center gap-1 rounded-md px-1 outline-none focus-visible:ring-2 focus-visible:ring-ring" aria-label={t("apps.switch")}>
        {name}
        <ChevronDown aria-hidden className="size-4 text-ink-soft" />
      </DropdownMenuTrigger>
      <DropdownMenuContent className="w-56">
        <DropdownMenuItem asChild>
          <Link href="/" aria-current="true">
            <Check aria-hidden className="size-4" />
            {t("app.name")}
          </Link>
        </DropdownMenuItem>
        <DropdownMenuItem asChild>
          <a href={new URL(accountCenter).origin}>
            <span aria-hidden className="size-4" />
            evinvest.ltd
          </a>
        </DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}

/** Review-archive tokens, never called a balance: evinvest.ltd's wallet is another ledger. Absent when unknown. */
function TokensStat() {
  const t = useT();
  const tokens = useTokens();
  if (tokens === null) return null;
  return (
    <a
      href={ROUTES.reviewArchiveTokens}
      className={cn("text-sm tabular-nums hover:underline", tokens.balance > 0 ? "text-ink" : "text-ink-soft")}
      title={t("account.tokens.renewal", { daily: tokens.daily, cap: tokens.cap })}
    >
      {t("tokens.count", { n: tokens.balance })}
    </a>
  );
}
