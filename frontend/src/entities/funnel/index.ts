export { fetchFunnel, fetchFunnelByLocation } from "./api/funnel";
export type { FunnelFilter } from "./api/funnel";
export { FUNNEL_STAGES, biggestLoss, funnelByLocationParser, funnelParser } from "./model/funnel";
export type { Funnel, FunnelByLocation, FunnelStage, FunnelStep, LocationSlice, Paid } from "./model/funnel";
export { INTENT_CHANNELS, aggregateParser } from "./model/aggregate";
export type { Aggregate, ContactIntents, IntentChannel, SiteVisits } from "./model/aggregate";
