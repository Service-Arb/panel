import { Suspense } from "react";

import { ExperimentsView } from "@/views/experiments";

export default function Page() {
  // The view reads the query string, which a static export only has in the browser.
  return (
    <Suspense>
      <ExperimentsView />
    </Suspense>
  );
}
