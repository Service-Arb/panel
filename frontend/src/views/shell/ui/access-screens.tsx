"use client";

import { Button, Empty, EmptyContent, EmptyDescription, EmptyHeader, EmptyTitle, Skeleton } from "@evinvest/uikit";

import { useT } from "@/shared/i18n";
import { useButtonSize } from "@/shared/ui/touch";

/** 403 from the gate: signed in to EV, but no grant on `allocation:service_arb`. */
export function NoAccessScreen() {
  const t = useT();
  return (
    <Empty className="min-h-svh">
      <EmptyHeader>
        <EmptyTitle>{t("state.noAccess.title")}</EmptyTitle>
        <EmptyDescription>{t("state.noAccess.body")}</EmptyDescription>
      </EmptyHeader>
    </Empty>
  );
}

/** 503: concierge unreachable. The session is kept, so this only offers to try again. */
export function UnavailableScreen({ onRetry }: { onRetry: () => void }) {
  const t = useT();
  const button = useButtonSize();
  return (
    <Empty className="min-h-svh">
      <EmptyHeader>
        <EmptyTitle>{t("state.unavailable.title")}</EmptyTitle>
        <EmptyDescription>{t("state.unavailable.body")}</EmptyDescription>
      </EmptyHeader>
      <EmptyContent>
        <Button size={button()} onClick={onRetry}>{t("state.retry")}</Button>
      </EmptyContent>
    </Empty>
  );
}

export function ShellSkeleton() {
  const t = useT();
  return (
    <div className="flex min-h-svh" aria-busy="true" aria-label={t("state.loading")}>
      <Skeleton className="hidden h-svh w-64 md:block" />
      <div className="flex flex-1 flex-col gap-3 p-6">
        <Skeleton className="h-8 w-48" />
        <Skeleton className="h-32 w-full" />
        <Skeleton className="h-32 w-full" />
      </div>
    </div>
  );
}
