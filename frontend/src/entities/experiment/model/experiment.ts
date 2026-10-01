import { aggregateSourceParser } from "@/shared/lib/aggregate-source";
import { type Infer, arrayOf, bool, nullable, num, object, oneOf, str } from "@/shared/lib/parse";
import { shareParser } from "@/shared/lib/share";

/**
 * The difference of a variant's rate and the control's, in percentage points
 * (variant − control), and its 95 % interval — rounded by the backend to
 * `decimals` (whole points for a wide interval, tenths for a narrow one).
 */
const differenceParser = object({ estimate: num, low: num, high: num, decimals: num });
export type Difference = Infer<typeof differenceParser>;

/**
 * A variant against the control on one rate. `difference` is null while an arm is
 * under `min_exposures` (`small_sample`); with it, `insufficient` still holds while
 * the interval contains zero. There is no winner field, and none is drawn here.
 */
const comparisonParser = object({
  difference: nullable(differenceParser),
  insufficient: bool,
  reason: nullable(oneOf(["small_sample", "interval_includes_zero"])),
});
export type Comparison = Infer<typeof comparisonParser>;

export const RATES = ["lead", "contact"] as const;
export type Rate = (typeof RATES)[number];

const variantParser = object({
  variant: str,
  control: bool,
  exposures: num,
  leads: num,
  intents: object({ phone: num, whatsapp: num, form_open: num, booking: num }),
  rates: object({ lead: shareParser, contact: shareParser }),
  vs_control: nullable(object({ lead: comparisonParser, contact: comparisonParser })),
});
export type Variant = Infer<typeof variantParser>;

const experimentParser = object({
  brand: str,
  experiment: str,
  first_day: str,
  last_day: str,
  control: str,
  variants: arrayOf(variantParser),
});
export type Experiment = Infer<typeof experimentParser>;

/** `GET /experiments`: the control first in each experiment's variants. */
export const experimentsParser = object({
  from: str,
  to: str,
  brand: nullable(str),
  min_sample: num,
  min_exposures: num,
  confidence: num,
  source: aggregateSourceParser,
  experiments: arrayOf(experimentParser),
});
export type Experiments = Infer<typeof experimentsParser>;
