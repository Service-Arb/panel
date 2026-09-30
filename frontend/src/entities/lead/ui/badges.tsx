"use client";

import { Badge, type BadgeVariant } from "@evinvest/uikit";

import { useT } from "@/shared/i18n";
import { formatWait } from "@/shared/lib/format";
import { useNow } from "@/shared/lib/use-now";

import { type Lead, type Stage, slaAt } from "../model/lead";

const STAGE_VARIANT: Record<Stage, BadgeVariant> = {
  created: "primary",
  contacted: "secondary",
  quoted: "secondary",
  won: "secondary",
  completed: "secondary",
  paid: "success",
  lost: "outline",
};

export function StageBadge({ stage }: { stage: Stage }) {
  const t = useT();
  return <Badge variant={STAGE_VARIANT[stage]}>{t(`stage.${stage}`)}</Badge>;
}

/** The first-contact SLA: red once overdue, nothing once the lead was reached. Ticks every minute. */
export function SlaBadge({ sla }: { sla: Lead["sla"] }) {
  const t = useT();
  const now = useNow();
  if (!sla) return null;
  const { seconds, overdue } = slaAt(sla, now);
  const time = formatWait(seconds, t);
  return overdue ? (
    <Badge variant="destructive">{t("leads.overdue", { time })}</Badge>
  ) : (
    <Badge variant="outline">{t("leads.waiting", { time })}</Badge>
  );
}
