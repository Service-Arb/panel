"use client";

import { Button, Item, ItemContent, ItemGroup, ItemTitle } from "@evinvest/uikit";
import { ArrowUpRight, FlaskConical, KeyRound, LayoutGrid } from "lucide-react";
import Link from "next/link";

import { managesSources, useMe } from "@/entities/session";
import { useSignOut } from "@/features/sign-out";
import { ROUTES } from "@/shared/config/routes";
import { useT } from "@/shared/i18n";
import { PageHeader } from "@/shared/ui/page-header";
import { useButtonSize } from "@/shared/ui/touch";

/** The phone's fourth tab: what the sidebar holds beyond the three screens. */
export function MoreView() {
  const t = useT();
  const button = useButtonSize();
  const me = useMe();
  const signOut = useSignOut();
  return (
    <div className="flex flex-col gap-4 p-4 md:p-6">
      <PageHeader title={t("more.title")} />
      <p className="text-sm text-ink-mid">
        {me.preferred_name || me.email} · {t(`nav.role.${me.role}`)}
      </p>
      <ItemGroup className="gap-2">
        <Item variant="outline" size="sm" asChild>
          <Link href={ROUTES.experiments}>
            <FlaskConical aria-hidden className="size-4" />
            <ItemContent>
              <ItemTitle>{t("nav.experiments")}</ItemTitle>
            </ItemContent>
          </Link>
        </Item>
        {managesSources(me.role) && (
          <Item variant="outline" size="sm" asChild>
            <Link href={ROUTES.sources}>
              <KeyRound aria-hidden className="size-4" />
              <ItemContent>
                <ItemTitle>{t("nav.sources")}</ItemTitle>
              </ItemContent>
            </Link>
          </Item>
        )}
        <Item variant="outline" size="sm" asChild>
          <a href={ROUTES.grafana} target="_blank" rel="noopener">
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
    </div>
  );
}
