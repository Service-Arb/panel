export { fetchExperiments } from "./api/experiments";
export type { ExperimentsFilter } from "./api/experiments";
export { formatPoints, readComparison } from "./lib/comparison";
export type { ComparisonReading, InsufficientReason } from "./lib/comparison";
export { AXIS_CAP, axisFor, placeInterval } from "./lib/interval-scale";
export { RATES, experimentsParser } from "./model/experiment";
export type { Comparison, Difference, Experiment, Experiments, Rate, Variant } from "./model/experiment";
export { IntervalChart } from "./ui/interval-chart";
