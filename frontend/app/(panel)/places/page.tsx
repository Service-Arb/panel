import { Suspense } from "react";

import { PlacesView } from "@/views/places";

export default function Page() {
  // The view reads the query string, which a static export only has in the browser.
  return (
    <Suspense>
      <PlacesView />
    </Suspense>
  );
}
