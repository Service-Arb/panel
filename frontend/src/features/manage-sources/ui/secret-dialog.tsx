"use client";

import { Button, Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@evinvest/uikit";
import { useState } from "react";

import type { AddedSource } from "@/entities/source";
import { useT } from "@/shared/i18n";
import { useButtonSize } from "@/shared/ui/touch";

/**
 * The new key's secret, once: the backend keeps it sealed and never returns it
 * again. Closing is an explicit "I saved it", not a click outside.
 */
export function SecretDialog({ added, onClose }: { added: AddedSource | null; onClose: () => void }) {
  const t = useT();
  const button = useButtonSize();
  const [copied, setCopied] = useState(false);

  const copy = async () => {
    if (!added) return;
    try {
      await navigator.clipboard.writeText(added.secret);
      setCopied(true);
    } catch {
      setCopied(false);
    }
  };

  const close = () => {
    setCopied(false);
    onClose();
  };

  return (
    <Dialog open={added !== null} onOpenChange={(open) => !open && close()}>
      <DialogContent showCloseButton={false}>
        <DialogHeader>
          <DialogTitle>{t("sources.secret.title", { key: added?.key_id ?? "" })}</DialogTitle>
          <DialogDescription>{t("sources.secret.body")}</DialogDescription>
        </DialogHeader>
        <code className="block break-all rounded-md border border-border bg-muted p-3 font-mono text-sm text-ink select-all">{added?.secret}</code>
        <DialogFooter>
          <Button variant="outline" size={button()} onClick={copy}>
            {copied ? t("sources.secret.copied") : t("sources.secret.copy")}
          </Button>
          <Button size={button()} onClick={close}>{t("sources.secret.done")}</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
