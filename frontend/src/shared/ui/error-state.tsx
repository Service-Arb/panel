"use client";

import { Alert, AlertDescription, Button } from "@evinvest/uikit";

import type { ApiFailure } from "@/shared/api";
import { useT } from "@/shared/i18n";

import { failureText } from "./failure-text";
import { useButtonSize } from "./touch";

export function ErrorState({ failure, onRetry }: { failure: ApiFailure | { kind: "invalid"; message: string }; onRetry?: () => void }) {
  const t = useT();
  const button = useButtonSize();
  return (
    <Alert variant="destructive" className="flex flex-col gap-3">
      <AlertDescription>{failureText(failure, t)}</AlertDescription>
      {onRetry && (
        <Button variant="outline" size={button("sm")} className="self-start" onClick={onRetry}>
          {t("state.retry")}
        </Button>
      )}
    </Alert>
  );
}
