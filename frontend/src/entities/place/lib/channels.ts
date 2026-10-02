import type { EditedFields } from "../model/settings";
import { type LocalTime, isOpenAt } from "./hours";

/** The ways a visitor reaches the place: a call, WhatsApp, or the form asking to be called back. */
export type Channel = "call" | "whatsapp" | "callback";

export interface ChannelSlot {
  channel: Channel;
  /** The panel's number; null where the site keeps its own (unknown here) or, for the form, none applies. */
  value: string | null;
  /** Set in the panel, or left to the site's own config. */
  from: "panel" | "site";
}

export interface ChannelPlan {
  /** "unknown" when the hours are the site's own, which the panel does not see. */
  state: "open" | "closed" | "unknown";
  slots: ChannelSlot[];
}

const OPEN_ORDER: readonly Channel[] = ["call", "whatsapp", "callback"];
const CLOSED_ORDER: readonly Channel[] = ["callback", "whatsapp", "call"];

/**
 * What the site will put in front of a visitor at `at`: open → the call first,
 * closed → the callback form and WhatsApp first, as nobody answers the phone.
 * Only what the panel knows is named; a channel the site supplies itself is
 * listed as the site's own, never filled in.
 */
export function channelPreview(fields: EditedFields, at: LocalTime): ChannelPlan {
  const state = fields.hours && fields.hours.length > 0 ? (isOpenAt(fields.hours, at) ? "open" : "closed") : "unknown";
  const slot = (channel: Channel): ChannelSlot => {
    if (channel === "callback") return { channel, value: null, from: "panel" };
    const value = channel === "call" ? fields.phone : fields.whatsapp;
    return value ? { channel, value, from: "panel" } : { channel, value: null, from: "site" };
  };
  return { state, slots: (state === "closed" ? CLOSED_ORDER : OPEN_ORDER).map(slot) };
}
