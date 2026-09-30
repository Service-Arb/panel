import { Suspense } from "react";

import { SourcesView } from "@/views/sources";

export default function Page() {
  // The view reads the query string, which a static export only has in the browser.
  return (
    <Suspense>
      <SourcesView />
    </Suspense>
  );
}
