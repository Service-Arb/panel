import { Suspense } from "react";

import { AccessView } from "@/views/access";

export default function AccessPage() {
  // Outside the shell: whoever lands here may hold nothing it shows. The view reads the query string.
  return (
    <Suspense>
      <AccessView />
    </Suspense>
  );
}
