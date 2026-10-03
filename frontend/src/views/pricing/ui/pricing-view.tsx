"use client";

import { Settled, Skeleton } from "@evinvest/uikit";
import { usePathname, useRouter, useSearchParams } from "next/navigation";
import { useState } from "react";

import { type PricingItem, fetchPricingList, followsPricing } from "@/entities/pricing";
import { ROUTES } from "@/shared/config/routes";
import { useT } from "@/shared/i18n";
import { utcDay } from "@/shared/lib/format";
import { useResource } from "@/shared/lib/use-resource";
import { EmptyState } from "@/shared/ui/empty-state";
import { ErrorState } from "@/shared/ui/error-state";
import { ScreenFrame } from "@/shared/ui/screen-frame";

import { BrandPricing } from "./brand-pricing";
import { BrandSelect } from "./brand-select";

/**
 * A brand's prices as its sites quote them: the model in the panel (or none:
 * the sites' baked one), its editor for an admin, a preview priced by the
 * server, the JSON way in and out, and the history.
 */
export function PricingView() {
  const t = useT();
  const router = useRouter();
  const pathname = usePathname();
  const params = useSearchParams();
  // Fixed for the page's life: a new empty draft's "valid from" must not move under it at midnight.
  const [today] = useState(() => utcDay(new Date()));
  const list = useResource("pricing:list", fetchPricingList, "pricing:list", { live: followsPricing(null) });
  const items: PricingItem[] = list.status === "ok" ? list.data : [];
  const wanted = params.get("brand");
  const brand = items.find((i) => i.brand_id === wanted)?.brand_id ?? items[0]?.brand_id ?? null;
  const choose = (b: string) => router.replace(`${pathname}?${new URLSearchParams({ brand: b }).toString()}`, { scroll: false });

  return (
    <ScreenFrame title={t("pricing.title")} back={ROUTES.more} actions={brand !== null ? <BrandSelect brands={items.map((i) => i.brand_id)} value={brand} onChange={choose} /> : undefined}>
      <Settled loading={list.status === "loading"} skeleton={<Skeleton className="h-64 w-full" />} className="flex flex-col gap-4">
        {list.status === "error" && <ErrorState failure={list.failure} onRetry={list.reload} />}
        {list.status === "ok" && brand === null && <EmptyState title={t("pricing.noBrands")} description={t("pricing.noBrands.body")} />}
        {brand !== null && <BrandPricing key={brand} brand={brand} today={today} />}
      </Settled>
    </ScreenFrame>
  );
}
