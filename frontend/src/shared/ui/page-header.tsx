import type { ReactNode } from "react";

/** A screen's title and, beside it (below it on a phone), its controls. */
export function PageHeader({ title, children }: { title: string; children?: ReactNode }) {
  return (
    <header className="flex flex-col gap-3 md:flex-row md:items-center md:justify-between">
      <h1 className="text-xl font-semibold text-ink">{title}</h1>
      {children}
    </header>
  );
}
