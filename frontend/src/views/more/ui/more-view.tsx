"use client";

import { Button, Item, ItemActions, ItemContent, ItemGroup, ItemTitle, NavDot } from "@evinvest/uikit";
import { ArrowUpRight, FlaskConical, KeyRound, LayoutGrid, type LucideIcon, Tags } from "lucide-react";
import Link from "next/link";

import { DevSignInBadge, managesSources, useMe } from "@/entities/session";
import { useSignOut } from "@/features/sign-out";
import { ROUTES } from "@/shared/config/routes";
import { type MessageKey, useT } from "@/shared/i18n";
import { useNavMark } from "@/shared/lib/nav-marks";
import { LiveStatusIndicator } from "@/shared/ui/live-status";
import { ScreenFrame } from "@/shared/ui/screen-frame";
import { useButtonSize } from "@/shared/ui/touch";

/** The phone's fourth tab: what the rail holds beyond the three screens, and who is signed in. */
export function MoreView() {
  const t = useT();
  const button = useButtonSize();
  const me = useMe();
  const signOut = useSignOut();
  return (
    <ScreenFrame title={t("more.title")}>
      <section className="flex flex-col gap-1" aria-label={t("nav.account")}>
        <span className="flex flex-wrap items-center gap-2 text-sm text-ink">
          {me.preferred_name || me.email} <DevSignInBadge />
        </span>
        <span className="text-sm text-ink-mid">{t(`nav.role.${me.role}`)}</span>
        <LiveStatusIndicator className="mt-1" />
      </section>
      <ItemGroup className="gap-2">
        <MoreLink href={ROUTES.pricing} icon={Tags} label="nav.pricing" />
        <MoreLink href={ROUTES.experiments} icon={FlaskConical} label="nav.experiments" />
        {managesSources(me.role) && <MoreLink href={ROUTES.sources} icon={KeyRound} label="nav.sources" />}
        <Item variant="outline" size="sm" asChild>
          <a href={ROUTES.grafana} target="_blank" rel="noopener noreferrer">
            <LayoutGrid aria-hidden className="size-4" />
            <ItemContent>
              <ItemTitle>{t("nav.grafana")}</ItemTitle>
            </ItemContent>
            <ArrowUpRight aria-hidden className="size-4" />
          </a>
        </Item>
      </ItemGroup>
      <Button variant="outline" size={button()} className="self-start" onClick={signOut}>
        {t("nav.signOut")}
      </Button>
    </ScreenFrame>
  );
}

function MoreLink({ href, icon: Icon, label }: { href: string; icon: LucideIcon; label: MessageKey }) {
  const t = useT();
  const mark = useNavMark(href);
  const changed = mark === true || (typeof mark === "number" && mark > 0);
  return (
    <Item variant="outline" size="sm" asChild>
      <Link href={href}>
        <Icon aria-hidden className="size-4" />
        <ItemContent>
          <ItemTitle>{t(label)}</ItemTitle>
        </ItemContent>
        {changed && (
          <ItemActions>
            <NavDot label={t("nav.changed")} />
          </ItemActions>
        )}
      </Link>
    </Item>
  );
}
