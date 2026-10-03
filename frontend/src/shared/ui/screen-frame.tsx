"use client";

import { MobileAppBar, PageFrame, type PageFrameWidth } from "@evinvest/uikit";
import Link from "next/link";
import type { ReactNode } from "react";

import { useT } from "@/shared/i18n";
import { DESKTOP_QUERY, useMediaQuery } from "@/shared/lib/use-media-query";

import { LiveStatusIndicator } from "./live-status";

export interface ScreenFrameProps {
  title: string;
  description?: string;
  /** The screen's controls: beside the heading on desktop, a row under the app bar on a phone. */
  actions?: ReactNode;
  /** A screen pushed from another (Sources from More on a phone): where the app bar's back goes. */
  back?: string;
  width?: PageFrameWidth;
  children: ReactNode;
}

/**
 * Every screen's frame: the kit's inset and title scale, sections arriving in
 * sequence, and on a phone an app bar with the live dot in place of the heading.
 * The actions are rendered once, wherever the width puts them — a second,
 * CSS-hidden copy would carry its own dialogs and state.
 */
export function ScreenFrame({ title, description, actions, back, width = "full", children }: ScreenFrameProps) {
  const t = useT();
  const isDesktop = useMediaQuery(DESKTOP_QUERY);
  const appBar = (
    <MobileAppBar
      title={title}
      hideFrom="md"
      linkComponent={Link}
      labels={{ back: t("nav.back") }}
      right={<LiveStatusIndicator compact />}
      {...(back === undefined ? {} : { back: { href: back } })}
    />
  );
  return (
    <PageFrame breakpoint="md" width={width} title={title} description={description} actions={isDesktop ? actions : undefined} appBar={appBar}>
      {!isDesktop && actions !== undefined && <div className="flex flex-col gap-3">{actions}</div>}
      {children}
    </PageFrame>
  );
}
