import { Empty, EmptyContent, EmptyDescription, EmptyHeader, EmptyTitle, cn } from "@evinvest/uikit";
import type { ReactNode } from "react";

/** The kit's Empty with its usual parts: what is missing, why, and what to do about it. */
export function EmptyState({ title, description, children, className }: { title: string; description?: string; children?: ReactNode; className?: string }) {
  return (
    <Empty className={cn("border border-dashed border-border", className)}>
      <EmptyHeader>
        <EmptyTitle>{title}</EmptyTitle>
        {description && <EmptyDescription>{description}</EmptyDescription>}
      </EmptyHeader>
      {children && <EmptyContent className="flex flex-row flex-wrap justify-center gap-2">{children}</EmptyContent>}
    </Empty>
  );
}
