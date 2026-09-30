"use client";

import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
  AlertDialogTrigger,
  Button,
  buttonVariants,
  toast,
} from "@evinvest/uikit";

import { revokeSource } from "@/entities/source";
import { useT } from "@/shared/i18n";
import { notifyFailure } from "@/shared/ui/notify";
import { TOUCH_TARGET } from "@/shared/ui/touch";

export function RevokeButton({ keyId, onRevoked }: { keyId: string; onRevoked: () => void }) {
  const t = useT();
  const revoke = async () => {
    try {
      await revokeSource(keyId);
      toast.positive(t("sources.revoked.toast"));
      onRevoked();
    } catch (e) {
      notifyFailure(e, t);
    }
  };
  return (
    <AlertDialog>
      <AlertDialogTrigger asChild>
        <Button variant="ghost" size="sm" className={TOUCH_TARGET}>
          {t("sources.revoke")}
        </Button>
      </AlertDialogTrigger>
      <AlertDialogContent>
        <AlertDialogHeader>
          <AlertDialogTitle>{t("sources.revoke.title", { key: keyId })}</AlertDialogTitle>
          <AlertDialogDescription>{t("sources.revoke.body")}</AlertDialogDescription>
        </AlertDialogHeader>
        <AlertDialogFooter>
          <AlertDialogCancel className={TOUCH_TARGET}>{t("move.cancel")}</AlertDialogCancel>
          <AlertDialogAction className={buttonVariants({ variant: "destructive", className: TOUCH_TARGET })} onClick={() => void revoke()}>
            {t("sources.revoke.confirm")}
          </AlertDialogAction>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  );
}
