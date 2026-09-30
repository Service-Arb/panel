import { http } from "@/shared/api";

import { type Funnel, funnelParser } from "../model/funnel";

export interface FunnelFilter {
  from: string;
  to: string;
  brand: string | null;
}

export function fetchFunnel(filter: FunnelFilter): Promise<Funnel> {
  return http.get("/api/v1/funnel", funnelParser, { from: filter.from, to: filter.to, brand: filter.brand });
}
