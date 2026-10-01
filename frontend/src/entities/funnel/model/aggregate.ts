import { type Infer, type Parser, ParseError, arrayOf, dictOf, num, object, record, str } from "@/shared/lib/parse";

/** The ways a visitor showed they meant to get in touch, as the landings report them. */
export const INTENT_CHANNELS = ["phone", "whatsapp", "form_open", "booking"] as const;
export type IntentChannel = (typeof INTENT_CHANNELS)[number];

const channelsParser = object({ phone: num, whatsapp: num, form_open: num, booking: num });

const visitsParser = object({
  total: num,
  by_source: dictOf(num),
  days: arrayOf(object({ day: str, n: num })),
});
export type SiteVisits = Infer<typeof visitsParser>;

const intentsParser = object({
  total: num,
  by_channel: channelsParser,
  days: arrayOf(object({ day: str, n: num, by_channel: channelsParser })),
});
export type ContactIntents = Infer<typeof intentsParser>;

export interface Aggregate {
  visits: SiteVisits;
  intents: ContactIntents;
}

/**
 * A slice's stages 3–4, picked out of `aggregate.stages` by name. Stages 1–2 (Maps)
 * will join the same list with the GBP import; until the front reads them, an
 * unknown stage is skipped rather than refused.
 */
export const aggregateParser: Parser<Aggregate> = (v, path) => {
  const stages = arrayOf(record)(record(v, path).stages, `${path}.stages`);
  const find = (name: string) => {
    const i = stages.findIndex((s) => s.stage === name);
    if (i < 0) throw new ParseError(`${path}.stages: no ${name}`);
    return { value: stages[i], at: `${path}.stages[${i}]` };
  };
  const visits = find("site.visit");
  const intents = find("contact.intent");
  return { visits: visitsParser(visits.value, visits.at), intents: intentsParser(intents.value, intents.at) };
};
