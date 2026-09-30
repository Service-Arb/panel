import { Suspense } from "react";

import { LeadsView } from "@/views/leads";

export default function Page() {
  // The view reads the query string, which a static export only has in the browser.
  return (
    <Suspense>
      <LeadsView />
    </Suspense>
  );
}
