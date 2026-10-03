"use client";

import { Badge, type BadgeVariant, cn } from "@evinvest/uikit";

import { useT } from "@/shared/i18n";
import { formatWait } from "@/shared/lib/format";
import { useNow } from "@/shared/lib/use-now";

import { type Lead, type Stage, type Suspect, slaAt } from "../model/lead";

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

/**
 * The antispam's doubt, said where the lead is read so nobody calls it in
 * ignorance. `short` (a list row) keeps the word; the reason rides as its title.
 */
export function SuspectBadge({ suspect, short = false }: { suspect: Suspect | null; short?: boolean }) {
  const t = useT();
  if (suspect === null) return null;
  const full = t("suspect.badge", { reason: t(`suspect.reason.${suspect}`) });
  return (
    <Badge variant="outline" title={full} className={cn("border-accent-warn bg-accent-warn/15 text-accent-warn", !short && "whitespace-normal")}>
      {short ? t("suspect.short") : full}
      {short && <span className="sr-only">{`: ${t(`suspect.reason.${suspect}`)}`}</span>}
    </Badge>
  );
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
