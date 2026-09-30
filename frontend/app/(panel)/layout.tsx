import type { ReactNode } from "react";

import { PanelShell } from "@/views/shell";

export default function PanelLayout({ children }: { children: ReactNode }) {
  return <PanelShell>{children}</PanelShell>;
}
