import type { ReactNode } from "react";

/** One titled part of the lead card. */
export function CardSection({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section className="flex flex-col gap-2">
      <h3 className="text-xs font-medium uppercase tracking-wide text-ink-soft">{title}</h3>
      {children}
    </section>
  );
}

/** Label and value pairs, two columns; values are text as the API gave them. */
export function FactList({ rows }: { rows: { label: string; value: ReactNode; mono?: boolean }[] }) {
  return (
    <dl className="grid grid-cols-[auto_1fr] gap-x-4 gap-y-1 text-sm">
      {rows.map(({ label, value, mono }) => (
        <div key={label} className="contents">
          <dt className={mono ? "font-mono text-xs text-ink-soft" : "text-ink-soft"}>{label}</dt>
          <dd className="min-w-0 wrap-anywhere text-ink">{value}</dd>
        </div>
      ))}
    </dl>
  );
}
