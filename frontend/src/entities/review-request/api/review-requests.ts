import { http } from "@/shared/api";

import { type ReviewRequests, reviewRequestsParser } from "../model/review-request";

export interface ReviewRequestsFilter {
  from: string;
  to: string;
  brand: string | null;
}

export function fetchReviewRequests(filter: ReviewRequestsFilter): Promise<ReviewRequests> {
  return http.get("/api/v1/review-requests", reviewRequestsParser, { from: filter.from, to: filter.to, brand: filter.brand });
}
