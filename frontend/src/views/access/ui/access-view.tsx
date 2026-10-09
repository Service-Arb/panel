"use client";

import { Empty, EmptyDescription, EmptyHeader, EmptyTitle } from "@evinvest/uikit";
import { useSearchParams } from "next/navigation";

import { NEEDS, RequestAccess, safeContinue } from "@/features/request-access";
import { useT } from "@/shared/i18n";

/** `/access?need=&continue=`: where a service behind the panel sends someone who lacks `need`. */
export function AccessView() {
  const t = useT();
  const params = useSearchParams();
  const need = NEEDS.find((n) => n === params.get("need"));
  if (need === undefined)
    return (
      <Empty className="min-h-svh">
        <EmptyHeader>
          <EmptyTitle>{t("access.invalid.title")}</EmptyTitle>
          <EmptyDescription>{t("access.invalid.body")}</EmptyDescription>
        </EmptyHeader>
      </Empty>
    );
  return <RequestAccess need={need} continueTo={safeContinue(params.get("continue"))} />;
}
