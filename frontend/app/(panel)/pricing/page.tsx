import { Suspense } from "react";

import { PricingView } from "@/views/pricing";

export default function Page() {
  // The view reads the query string (`?brand=`), which a static export only has in the browser.
  return (
    <Suspense>
      <PricingView />
    </Suspense>
  );
}
