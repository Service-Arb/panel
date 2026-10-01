import { http } from "@/shared/api";

import { type Experiments, experimentsParser } from "../model/experiment";

export interface ExperimentsFilter {
  from: string;
  to: string;
  brand: string | null;
}

export function fetchExperiments(filter: ExperimentsFilter): Promise<Experiments> {
  return http.get("/api/v1/experiments", experimentsParser, { from: filter.from, to: filter.to, brand: filter.brand });
}
