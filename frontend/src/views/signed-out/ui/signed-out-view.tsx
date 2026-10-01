"use client";

import { Button, Empty, EmptyContent, EmptyDescription, EmptyHeader, EmptyTitle } from "@evinvest/uikit";

import { SIGN_IN_PATH } from "@/shared/api";
import { useT } from "@/shared/i18n";
import { useButtonSize } from "@/shared/ui/touch";

/** After "Sign out": a page outside the shell, so it does not bounce straight back to sign-in. */
export function SignedOutView() {
  const t = useT();
  const button = useButtonSize();
  return (
    <Empty className="min-h-svh">
      <EmptyHeader>
        <EmptyTitle>{t("state.signedOut.title")}</EmptyTitle>
        <EmptyDescription>{t("state.signedOut.body")}</EmptyDescription>
      </EmptyHeader>
      <EmptyContent>
        <Button asChild size={button()}>
          <a href={SIGN_IN_PATH}>{t("state.signedOut.again")}</a>
        </Button>
      </EmptyContent>
    </Empty>
  );
}
