import { Suspense } from "react";

import { MoreView } from "@/views/more";

export default function Page() {
  // The view reads the query string, which a static export only has in the browser.
  return (
    <Suspense>
      <MoreView />
    </Suspense>
  );
}
