import { describe, expect, it, vi } from "vitest";

import type { Lead, LeadEvent } from "@/entities/lead";
import { NO_BOOKING } from "@/entities/lead/model/booking";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";

import { type AskDeps, type AskPlan, askForReview, leftoverFor } from "@/features/request-review/model/ask";
import { reviewGate } from "@/features/request-review/model/gate";
import { telegramHandleOf, telegramUsername, telegramUrl, whatsappUrl } from "@/features/request-review/model/links";
import { brandLabel, firstNameOf, messageLocale, reviewMessage } from "@/features/request-review/model/message";
import { reviewPlans } from "@/features/request-review/model/plans";
import { Leftover } from "@/features/request-review/ui/leftover";
import { ReviewTrigger } from "@/features/request-review/ui/review-trigger";

import en from "../messages/en.json";
import ru from "../messages/ru.json";

function lead(patch: Partial<Lead> = {}): Lead {
  return {
    brand: "aquafix", lead_id: "L-1", location: "lyon-3", job_id: null, stage: "completed", channel: "form", message_ref: null,
    messaged_at: null, messaged_channel: null, locale: null, review_requested_at: null, review_requested_channel: null, manual: false,
    created_at: "2026-10-01T10:00:00Z", contacted_at: null, quoted_at: null, won_at: null, completed_at: "2026-10-05T10:00:00Z", paid_at: null,
    lost_at: null, lost_reason: null, suspect: null, last_event_at: "2026-10-05T10:00:00Z", sla: null, pii: { name: "Jean Dupont", phone: "+33 6 12 34 56 78" },
    flow: null, quoted_cents: null, pricing_valid_from: null, estimate_inputs: null, booking: NO_BOOKING, ...patch,
  };
}

const WINDOW = {} as Window; // only its being non-null matters
const LINK = "https://g.page/r/abc/review";

describe("the message to the customer", () => {
  const parts = { name: "Jean Dupont", brand: "aquafix", link: LINK };

  it("is French for a French lead and for one with no language", () => {
    const fr = "Bonjour Jean, merci d'avoir fait appel à aquafix. Si vous avez une minute, votre avis nous aide beaucoup : " + LINK;
    expect(reviewMessage({ ...parts, locale: "fr" })).toBe(fr);
    expect(reviewMessage({ ...parts, locale: null })).toBe(fr);
    expect(messageLocale(null)).toBe("fr");
  });

  it("is English for an English lead", () => {
    expect(reviewMessage({ ...parts, locale: "en" })).toBe(`Hello Jean, thank you for choosing aquafix. If you have a minute, your review helps us a lot: ${LINK}`);
  });

  it("greets nobody by name when the customer left none", () => {
    expect(reviewMessage({ ...parts, name: null, locale: "fr" }).startsWith("Bonjour, merci")).toBe(true);
    expect(reviewMessage({ ...parts, name: "  ", locale: "en" }).startsWith("Hello, thank you")).toBe(true);
    expect(firstNameOf("  Marie  Curie")).toBe("Marie");
  });
});

describe("the brand in the message", () => {
  it("is the place's brandName", () => {
    expect(brandLabel("Aquafix Plomberie", "aquafix")).toBe("Aquafix Plomberie");
    expect(reviewPlans(lead(), [], LINK, brandLabel("Aquafix Plomberie", "aquafix"))[0]!.text).toContain("fait appel à Aquafix Plomberie.");
  });

  it("falls back to the slug, capitalised, when the place has none", () => {
    expect(brandLabel(null, "aquafix")).toBe("Aquafix");
    expect(brandLabel("  ", "vifnet")).toBe("Vifnet");
    expect(reviewPlans(lead({ locale: "en" }), [], LINK, brandLabel(null, "aquafix"))[0]!.text).toContain("choosing Aquafix.");
  });
});

describe("the links", () => {
  it("builds wa.me from the digits of the number, the text encoded", () => {
    const url = whatsappUrl("+33 6 12-34.56 78", "Bonjour, merci d'avoir fait appel : https://x.test/?a=1&b=2");
    expect(url).toBe(`https://wa.me/33612345678?text=${encodeURIComponent("Bonjour, merci d'avoir fait appel : https://x.test/?a=1&b=2")}`);
    expect(new URL(url!).searchParams.get("text")).toBe("Bonjour, merci d'avoir fait appel : https://x.test/?a=1&b=2");
  });

  it("has no WhatsApp chat without a dialable number", () => {
    expect(whatsappUrl(null, "x")).toBeNull();
    expect(whatsappUrl("call me", "x")).toBeNull();
  });

  it("takes only a username for Telegram, with or without the @", () => {
    expect(telegramUsername("@theo_stub")).toBe("theo_stub");
    expect(telegramUsername("theo_stub")).toBe("theo_stub");
    expect(telegramUsername("Théo Stub")).toBeNull();
    expect(telegramUsername("+33612345678")).toBeNull();
    expect(telegramUrl("theo_stub")).toBe("https://t.me/theo_stub");
  });

  it("reads the handle of the latest Telegram message", () => {
    const e = (properties: Record<string, unknown>, handle: unknown): LeadEvent => ({
      id: "e", type: "lead.messaged", type_version: 1, occurred_at: "", received_at: "", source_kind: "bot", source_id: "b", manual: false,
      job_id: null, status: "accepted", status_reason: null, properties, pii: { handle },
    });
    expect(telegramHandleOf([e({ channel: "whatsapp" }, "@wa_name"), e({ channel: "telegram" }, "@theo_stub")])).toBe("theo_stub");
    expect(telegramHandleOf([e({ channel: "whatsapp" }, "@wa_name")])).toBeNull();
    expect(telegramHandleOf([])).toBeNull();
  });
});

describe("the ways to ask", () => {
  const telegram = [{ type: "lead.messaged", properties: { channel: "telegram" }, pii: { handle: "@theo_stub" } } as unknown as LeadEvent];

  it("is WhatsApp's chat alone for a customer with a number", () => {
    const plans = reviewPlans(lead(), [], LINK, "aquafix");
    expect(plans.map((p) => p.channel)).toEqual(["whatsapp"]);
    expect(plans[0]!.url).toMatch(/^https:\/\/wa\.me\/33612345678\?text=/);
  });

  it("copies for WhatsApp when there is no number", () => {
    expect(reviewPlans(lead({ pii: { name: "Jean" } }), [], LINK, "aquafix")[0]).toMatchObject({ channel: "whatsapp", url: null });
  });

  it("offers both when the customer has a number and wrote on Telegram", () => {
    expect(reviewPlans(lead(), telegram, LINK, "aquafix").map((p) => p.channel)).toEqual(["whatsapp", "telegram"]);
  });

  it("offers Telegram alone to a Telegram customer with no number", () => {
    expect(reviewPlans(lead({ pii: {} }), telegram, LINK, "aquafix").map((p) => p.channel)).toEqual(["telegram"]);
  });

  it("writes in the lead's language", () => {
    expect(reviewPlans(lead({ locale: "en" }), [], LINK, "aquafix")[0]!.text).toMatch(/^Hello Jean,/);
    expect(reviewPlans(lead(), [], LINK, "aquafix")[0]!.text).toMatch(/^Bonjour Jean,/);
  });
});

describe("asking", () => {
  const whatsapp: AskPlan = { channel: "whatsapp", text: "hi", url: "https://wa.me/1?text=hi" };
  const tg: AskPlan = { channel: "telegram", text: "hi", url: "https://t.me/theo_stub" };
  const deps = (already = false) => {
    const calls: string[] = [];
    const d: AskDeps = {
      send: vi.fn(async (c) => (calls.push(`send:${c}`), { event_id: "e", already_requested: already })),
      open: vi.fn((u) => (calls.push(`open:${u}`), WINDOW)),
      copy: vi.fn(async (t) => void calls.push(`copy:${t}`)),
    };
    return { d, calls };
  };

  it("records the request before the chat opens", async () => {
    const { d, calls } = deps();
    expect(await askForReview(whatsapp, d)).toEqual({ kind: "opened" });
    expect(calls).toEqual(["send:whatsapp", "open:https://wa.me/1?text=hi"]);
  });

  it("copies the text, then opens Telegram, after the record", async () => {
    const { d, calls } = deps();
    expect(await askForReview(tg, d)).toEqual({ kind: "copied_and_opened" });
    expect(calls).toEqual(["send:telegram", "copy:hi", "open:https://t.me/theo_stub"]);
  });

  it("only copies for WhatsApp with no number", async () => {
    const { d, calls } = deps();
    expect(await askForReview({ ...whatsapp, url: null }, d)).toEqual({ kind: "copied" });
    expect(calls).toEqual(["send:whatsapp", "copy:hi"]);
  });

  it("does not open or copy again for a review already asked", async () => {
    const { d } = deps(true);
    expect(await askForReview(whatsapp, d)).toEqual({ kind: "already" });
    expect(await askForReview(tg, d)).toEqual({ kind: "already" });
    expect(d.open).not.toHaveBeenCalled();
    expect(d.copy).not.toHaveBeenCalled();
  });

  it("opens and copies nothing when the server refuses", async () => {
    const { d } = deps();
    vi.mocked(d.send).mockRejectedValueOnce(new Error("409"));
    await expect(askForReview(whatsapp, d)).rejects.toThrow("409");
    expect(d.open).not.toHaveBeenCalled();
    expect(d.copy).not.toHaveBeenCalled();
  });

  it("hands the text over when the clipboard refuses, and the link when the window is blocked", async () => {
    const { d } = deps();
    vi.mocked(d.copy).mockRejectedValueOnce(new Error("denied"));
    expect(await askForReview(tg, d)).toEqual({ kind: "copy_failed", text: "hi", url: "https://t.me/theo_stub" });
    vi.mocked(d.open).mockReturnValueOnce(null);
    expect(await askForReview(whatsapp, d)).toEqual({ kind: "blocked", url: whatsapp.url });
  });
});

describe("when the card offers it", () => {
  it("shows the button for a completed or paid job at a place with a review link", () => {
    expect(reviewGate(lead(), LINK, true)).toEqual({ kind: "ready" });
    expect(reviewGate(lead({ stage: "paid", completed_at: null, paid_at: "2026-10-06T10:00:00Z" }), LINK, true)).toEqual({ kind: "ready" });
  });

  it("still shows it for a lead lost after the job was done", () => {
    expect(reviewGate(lead({ stage: "lost", lost_at: "2026-10-07T10:00:00Z" }), LINK, true)).toEqual({ kind: "ready" });
  });

  it("hides it before the job was done, whatever the stage says", () => {
    expect(reviewGate(lead({ stage: "created", completed_at: null }), LINK, true)).toEqual({ kind: "hidden" });
    expect(reviewGate(lead({ stage: "won", completed_at: null, won_at: "2026-10-03T10:00:00Z" }), LINK, true)).toEqual({ kind: "hidden" });
  });

  it("hides it where the place has no review link", () => {
    expect(reviewGate(lead(), null, true)).toEqual({ kind: "hidden" });
    expect(reviewGate(lead(), "", true)).toEqual({ kind: "hidden" });
  });

  it("is off, with its reason, without the right to see contacts", () => {
    expect(reviewGate(lead(), LINK, false)).toEqual({ kind: "needs_pii" });
  });

  it("says when it was asked, and offers nothing more", () => {
    const asked = lead({ review_requested_at: "2026-10-08T09:00:00Z", review_requested_channel: "telegram" });
    expect(reviewGate(asked, LINK, true)).toEqual({ kind: "asked", at: "2026-10-08T09:00:00Z", channel: "telegram" });
    expect(reviewGate(asked, null, false)).toMatchObject({ kind: "asked" });
  });
});

describe("the catalogues", () => {
  it("name the review texts in both languages", () => {
    for (const key of ["review.ask", "review.needsPii", "review.requested"] as const) {
      expect(en[key]).toBeTruthy();
      expect(ru[key]).toBeTruthy();
    }
  });
});

describe("what is left for the person to do", () => {
  it("shows the text to select and the chat to open, in a status region, with a way to close it", () => {
    const html = renderToStaticMarkup(createElement(Leftover, { left: { kind: "copy_failed", text: "Bonjour Jean, merci : https://g.page/r/abc", url: "https://t.me/theo_stub" }, onClose: () => undefined }));
    expect(html).toContain('role="status"');
    expect(html).toContain("Bonjour Jean, merci : https://g.page/r/abc");
    expect(html).toContain('href="https://t.me/theo_stub"');
    expect(html).toContain("overflow-wrap:anywhere");
    expect(html).toContain("Dismiss");
    expect(html).not.toContain("Close");
  });

  it("is for a blocked window only the link", () => {
    const html = renderToStaticMarkup(createElement(Leftover, { left: { kind: "blocked", url: "https://wa.me/1?text=hi" }, onClose: () => undefined }));
    expect(html).toContain('href="https://wa.me/1?text=hi"');
    expect(html).not.toContain("<code");
  });

  it("stays for the lead it was held for through the card re-reading, and not for another", () => {
    const held = { leadId: "L-1", what: { kind: "copy_failed", text: "hi", url: null } as const };
    expect(leftoverFor(held, "L-1")).toBe(held.what); // the re-read lead has review_requested_at now; the held value is untouched
    expect(leftoverFor(held, "L-2")).toBeNull();
    expect(leftoverFor(null, "L-1")).toBeNull();
  });
});

describe("the button when it is off", () => {
  const render = (off: boolean) => renderToStaticMarkup(createElement(ReviewTrigger, { label: "Ask", off, reasonId: "why", size: "lg", onPress: () => undefined }));

  it("is aria-disabled and described by the reason, not natively disabled, so it keeps focus and the reason is read", () => {
    const html = render(true);
    expect(html).toContain('aria-disabled="true"');
    expect(html).toContain('aria-describedby="why"');
    expect(html).not.toMatch(/\sdisabled(=|\s|>)/);
  });

  it("is plain when on", () => {
    const html = render(false);
    expect(html).not.toContain('aria-describedby');
    expect(html).not.toContain('aria-disabled="true"');
  });
});

describe("the button as a menu's trigger", () => {
  // A trigger given `asChild` hands its toggle in as `onClick`; the button must keep it, whether or not it has a press of its own.
  const rendered = (props: Record<string, unknown>) => ReviewTrigger({ label: "Ask", off: false, reasonId: undefined, size: "lg", onPress: undefined, ...props }) as { props: { onClick: (e: unknown) => void } };

  it("still calls the onClick it is handed when it has no press of its own (two ways to ask)", () => {
    const toggle = vi.fn();
    rendered({ onClick: toggle }).props.onClick({});
    expect(toggle).toHaveBeenCalledOnce();
  });

  it("calls both the handed onClick and its own press, the handed one first", () => {
    const calls: string[] = [];
    rendered({ onClick: () => calls.push("toggle"), onPress: () => calls.push("press") }).props.onClick({});
    expect(calls).toEqual(["toggle", "press"]);
  });

  it("does not press when off, and passes the trigger's other props on", () => {
    const press = vi.fn();
    const el = rendered({ off: true, onPress: press, "aria-haspopup": "menu", "aria-expanded": false });
    el.props.onClick({});
    expect(press).not.toHaveBeenCalled();
    expect(el.props).toMatchObject({ "aria-haspopup": "menu", "aria-expanded": false });
  });
});
