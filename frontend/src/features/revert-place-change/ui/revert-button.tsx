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

import { type PlaceKey, type PlaceSettingsView, revertSettingsChange } from "@/entities/place";
import { useT } from "@/shared/i18n";
import { notifyFailure } from "@/shared/ui/notify";
import { useButtonSize } from "@/shared/ui/touch";

/** Puts a change's `before` back, after a confirmation that says the revert is itself recorded. */
export function RevertButton({ place, changeId, onReverted }: { place: PlaceKey; changeId: string; onReverted: (view: PlaceSettingsView) => void }) {
  const t = useT();
  const button = useButtonSize();
  const revert = async () => {
    try {
      const view = await revertSettingsChange(place, changeId);
      toast.positive(t("placeSettings.history.reverted"));
      onReverted(view);
    } catch (e) {
      notifyFailure(e, t);
    }
  };
  return (
    <AlertDialog>
      <AlertDialogTrigger asChild>
        <Button variant="outline" size={button("sm")}>
          {t("placeSettings.history.revert")}
        </Button>
      </AlertDialogTrigger>
      <AlertDialogContent>
        <AlertDialogHeader>
          <AlertDialogTitle>{t("placeSettings.history.revert.title")}</AlertDialogTitle>
          <AlertDialogDescription>{t("placeSettings.history.revert.body")}</AlertDialogDescription>
        </AlertDialogHeader>
        <AlertDialogFooter>
          <AlertDialogCancel className={buttonVariants({ variant: "outline", size: button() })}>{t("move.cancel")}</AlertDialogCancel>
          <AlertDialogAction className={buttonVariants({ variant: "primary", size: button() })} onClick={() => void revert()}>
            {t("placeSettings.history.revert")}
          </AlertDialogAction>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  );
}
