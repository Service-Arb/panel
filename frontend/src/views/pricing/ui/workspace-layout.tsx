import type { ReactNode } from "react";

/** The model and its editor; beside them from `xl` up (under them below) the preview, JSON and history. */
export function WorkspaceLayout({ main, aside }: { main: ReactNode; aside: ReactNode }) {
  return (
    <div className="grid items-start gap-6 xl:grid-cols-(--grid-pricing)">
      <div className="flex min-w-0 flex-col gap-6">{main}</div>
      <aside className="flex min-w-0 flex-col gap-4 xl:sticky xl:top-4">{aside}</aside>
    </div>
  );
}
