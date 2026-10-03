"use client";

import { Settled, Skeleton } from "@evinvest/uikit";
import { useCallback, useState } from "react";

import { type PricingItem, fetchPricing, followsPricing, pricingSavedSince } from "@/entities/pricing";
import { managesPricing, useMe } from "@/entities/session";
import { useResource } from "@/shared/lib/use-resource";
import { ErrorState } from "@/shared/ui/error-state";

import { EditWorkspace } from "./edit-workspace";
import { ReadWorkspace } from "./read-workspace";

/**
 * One brand's pricing. The editor works from `pinned` — the pricing as it was
 * when the draft started — while the read underneath follows live changes:
 * one's own save pins what it returned, someone else's shows up as `fresher`
 * and the editor decides what to do with it.
 */
export function BrandPricing({ brand, today }: { brand: string; today: string }) {
  const { role } = useMe();
  const [version, setVersion] = useState(0);
  const [pinned, setPinned] = useState<PricingItem | null>(null);
  const data = useResource(`pricing:${brand}:${version}`, () => fetchPricing(brand), `pricing:${brand}`, { live: followsPricing(brand) });
  const latest = data.status === "ok" ? data.data : null;
  if (latest !== null && pinned === null) setPinned(latest);

  /** After a write: `item` is what the server now holds; without one, read it again. */
  const written = useCallback((item?: PricingItem) => {
    setPinned(item ?? null);
    setVersion((v) => v + 1);
  }, []);
  const takeFresh = useCallback((item: PricingItem) => setPinned(item), []);

  return (
    <Settled loading={data.status === "loading"} skeleton={<Skeleton className="h-96 w-full" />}>
      {data.status === "error" && <ErrorState failure={data.failure} onRetry={data.reload} />}
      {latest !== null &&
        (managesPricing(role) ? (
          <EditWorkspace
            key={pinned?.updated_at ?? "never"}
            base={pinned ?? latest}
            fresher={pinned && pricingSavedSince(latest, pinned) ? latest : null}
            today={today}
            version={version}
            onWritten={written}
            onTakeFresh={takeFresh}
          />
        ) : (
          // Nothing typed to protect: a reader sees the newest at once.
          <ReadWorkspace item={latest} version={version} />
        ))}
    </Settled>
  );
}
