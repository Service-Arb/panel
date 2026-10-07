import { describe, expect, it } from "vitest";

import en from "../messages/en.json";
import ru from "../messages/ru.json";

import { CHANNELS, awaitingMessage, leadParser } from "@/entities/lead/model/lead";
import { sourceKindLabel } from "@/entities/source/lib/kind";
import { sourceParser } from "@/entities/source/model/source";
import { leadFilterFrom, messageRefOf, narrows, paramsWith } from "@/features/lead-filters/model/params";
import { translator } from "@/shared/i18n/translate";
import { parse } from "@/shared/lib/parse";

const row = { brand: "aquafix", lead_id: "l1", stage: "created", manual: false, last_event_at: "2026-10-07T10:00:00Z" };

describe("a messenger lead from the API", () => {
  it("reads a WhatsApp lead with its ref and the Telegram message that answered it", () => {
    const lead = parse(leadParser, { ...row, channel: "whatsapp", message_ref: "AQ-7K3F", messaged_at: "2026-10-07T10:05:00Z", messaged_channel: "telegram" });
    expect(lead).toMatchObject({ channel: "whatsapp", message_ref: "AQ-7K3F", messaged_at: "2026-10-07T10:05:00Z", messaged_channel: "telegram" });
  });

  it("refuses a channel the contract does not name, saying which field", () => {
    expect(() => parse(leadParser, { ...row, channel: "sms" })).toThrow(/^\$\.channel: /);
  });

  it("refuses a messaged channel that is not a messenger, saying which field", () => {
    expect(() => parse(leadParser, { ...row, channel: "whatsapp", messaged_at: "2026-10-07T10:05:00Z", messaged_channel: "phone" })).toThrow(/^\$\.messaged_channel: /);
  });

  it("reads a lead from before messengers, the fields absent, as none of them", () => {
    const lead = parse(leadParser, row);
    expect(lead).toMatchObject({ channel: null, message_ref: null, messaged_at: null, messaged_channel: null });
  });
});

describe("the messenger a lead waits on", () => {
  it("is WhatsApp for a WhatsApp lead the customer has not written on yet", () => {
    expect(awaitingMessage({ channel: "whatsapp", messaged_at: null })).toBe("whatsapp");
  });

  it("is none once the customer wrote", () => {
    expect(awaitingMessage({ channel: "telegram", messaged_at: "2026-10-07T10:05:00Z" })).toBeNull();
  });

  it("is none for a form lead or a lead without a channel", () => {
    expect(awaitingMessage({ channel: "form", messaged_at: null })).toBeNull();
    expect(awaitingMessage({ channel: null, messaged_at: null })).toBeNull();
  });
});

describe("the message ref an operator types", () => {
  it("is trimmed and upper-cased into the form the API matches", () => {
    expect(messageRefOf(" aq-7k3f ")).toBe("AQ-7K3F");
  });

  it("is none for a letter outside Crockford base32, a one-letter prefix, or nothing", () => {
    expect(messageRefOf("AQ-7K3I")).toBeNull();
    expect(messageRefOf("A-7K3F")).toBeNull();
    expect(messageRefOf("")).toBeNull();
    expect(messageRefOf(null)).toBeNull();
  });
});

describe("the channel and ref filters in the URL", () => {
  it("drop a channel the API does not take", () => {
    expect(leadFilterFrom(new URLSearchParams("channel=sms")).channel).toBeNull();
  });

  it("go into the URL and back out of it", () => {
    const params = paramsWith(new URLSearchParams("stage=created"), { channel: "telegram", messageRef: "AQ-7K3F" });
    expect(params.toString()).toBe("stage=created&channel=telegram&message_ref=AQ-7K3F");
    expect(leadFilterFrom(params)).toMatchObject({ stage: "created", channel: "telegram", messageRef: "AQ-7K3F" });
    expect(paramsWith(params, { channel: null, messageRef: null }).toString()).toBe("stage=created");
  });

  it("read a lower-case ref from a pasted link as the API matches it", () => {
    expect(leadFilterFrom(new URLSearchParams("message_ref=aq-7k3f")).messageRef).toBe("AQ-7K3F");
  });

  it("each narrow the list on its own", () => {
    const none = leadFilterFrom(new URLSearchParams(""));
    expect(narrows(none)).toBe(false);
    expect(narrows(leadFilterFrom(new URLSearchParams("channel=whatsapp")))).toBe(true);
    expect(narrows(leadFilterFrom(new URLSearchParams("message_ref=AQ-7K3F")))).toBe(true);
    expect(narrows(leadFilterFrom(new URLSearchParams("channel=whatsapp")), ["messageRef"])).toBe(false);
  });
});

describe("the channel badge's words", () => {
  const key = (channel: string) => `channel.${channel}`;
  const missing = (catalogue: Record<string, string>) => CHANNELS.map(key).filter((k) => !(k in catalogue));

  it("exist in English for every channel the contract names", () => {
    expect(missing(en)).toEqual([]);
  });

  it("exist in Russian for every channel the contract names", () => {
    expect(missing(ru)).toEqual([]);
  });
});

describe("the bot source", () => {
  it("is a kind the sources list reads, and says which bots it is", () => {
    const source = parse(sourceParser, { key_id: "k1", kind: "bot", brands: ["aquafix"], created_at: "2026-10-07T10:00:00Z", revoked_at: null });
    expect(sourceKindLabel(source.kind, translator("en"))).toBe("Bot (WhatsApp/Telegram)");
    expect(sourceKindLabel("site", translator("en"))).toBe("site");
  });
});
