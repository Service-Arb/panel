import { Suspense } from "react";

import { OverviewView } from "@/views/overview";

export default function Page() {
  // The view reads the query string, which a static export only has in the browser.
  return (
    <Suspense>
      <OverviewView />
    </Suspense>
  );
}
