"use client";

import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@evinvest/uikit";

import { type PricingItem, PricingStatus } from "@/entities/pricing";
import { ClearPricingButton } from "@/features/clear-pricing";
import { JsonTransfer, PricingEditorForm, usePricingEditor } from "@/features/edit-pricing";
import { PreviewPanel } from "@/features/preview-price";
import { useT } from "@/shared/i18n";

import { PricingHistory } from "./pricing-history";
import { WorkspaceLayout } from "./workspace-layout";

export interface EditWorkspaceProps {
  /** The pricing the draft starts from; re-mount on a new one. */
  base: PricingItem;
  fresher: PricingItem | null;
  today: string;
  version: number;
  onWritten: (item?: PricingItem) => void;
  onTakeFresh: (item: PricingItem) => void;
}

/** An admin's screen: the editor, and the preview of its draft as the server prices it. */
export function EditWorkspace({ base, fresher, today, version, onWritten, onTakeFresh }: EditWorkspaceProps) {
  const t = useT();
  const editor = usePricingEditor(base, today, onWritten);
  return (
    <WorkspaceLayout
      main={
        <>
          <PricingStatus item={base} action={<ClearPricingButton item={base} onCleared={() => onWritten()} />} />
          <PricingEditorForm editor={editor} fresher={fresher} onTakeFresh={onTakeFresh} onReload={() => onWritten()} />
        </>
      }
      aside={
        <>
          <PreviewPanel brand={base.brand_id} model={editor.serialized.model} onShowPath={editor.showPath} onShowProblem={editor.showFirst} />
          <Card>
            <CardHeader>
              <CardTitle>{t("pricing.json.title")}</CardTitle>
              <CardDescription>{t("pricing.json.body")}</CardDescription>
            </CardHeader>
            <CardContent>
              <JsonTransfer editor={editor} />
            </CardContent>
          </Card>
          <PricingHistory brand={base.brand_id} version={version} />
        </>
      }
    />
  );
}
