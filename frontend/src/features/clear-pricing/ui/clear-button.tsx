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

import { type PricingItem, clearPricing } from "@/entities/pricing";
import { ApiError } from "@/shared/api";
import { useT } from "@/shared/i18n";
import { notifyFailure } from "@/shared/ui/notify";
import { useButtonSize } from "@/shared/ui/touch";

/**
 * Admin only. Taking the model off sends every site of the brand back to the
 * prices built into it, so the confirmation says that in as many words.
 */
export function ClearPricingButton({ item, onCleared }: { item: PricingItem; onCleared: () => void }) {
  const t = useT();
  const button = useButtonSize();
  const run = async () => {
    try {
      await clearPricing(item.brand_id, item.updated_at);
      toast.positive(t("pricing.clear.done"));
    } catch (e) {
      // Someone saved first: what is current is read again rather than removed unseen.
      if (e instanceof ApiError && e.failure.kind === "conflict") toast.error(t("pricing.clear.conflict"));
      else notifyFailure(e, t);
    }
    onCleared();
  };
  return (
    <AlertDialog>
      <AlertDialogTrigger asChild>
        <Button variant="outline" size={button("sm")} disabled={item.model === null}>
          {t("pricing.clear")}
        </Button>
      </AlertDialogTrigger>
      <AlertDialogContent>
        <AlertDialogHeader>
          <AlertDialogTitle>{t("pricing.clear.title", { brand: item.brand_id })}</AlertDialogTitle>
          <AlertDialogDescription>{t("pricing.clear.body")}</AlertDialogDescription>
        </AlertDialogHeader>
        <AlertDialogFooter>
          <AlertDialogCancel className={buttonVariants({ variant: "outline", size: button() })}>{t("move.cancel")}</AlertDialogCancel>
          <AlertDialogAction className={buttonVariants({ variant: "destructive", size: button() })} onClick={() => void run()}>
            {t("pricing.clear")}
          </AlertDialogAction>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  );
}
