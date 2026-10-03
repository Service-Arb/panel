import { readFileSync, readdirSync } from "node:fs";

import { describe, expect, it } from "vitest";

import { bookingText } from "@/entities/place/lib/booking";
import { diffLineText, diffSettings } from "@/entities/place/lib/diff";
import { CAL_COM_HOSTS, bookingConfigOf, checkBookingConfig, checkBookingUrl } from "@/entities/place/model/booking";
import { placeSettingsParser, settingsParser, settingsToWire } from "@/entities/place/model/settings";
import { bookingDraftOf, bookingDraftProblems, bookingOf } from "@/features/edit-place-settings/model/booking-draft";
import { draftChanged, draftOf, draftValid, editedOf, settingsOf } from "@/features/edit-place-settings/model/draft";
import { fieldErrorsOf } from "@/features/edit-place-settings/model/field-errors";
import { translator } from "@/shared/i18n/translate";
import { parse } from "@/shared/lib/parse";

// The backend's own copy of kitstart's booking fixtures (vendored by sha in the
// same repository, `SOURCE`): read in place, so the panel's two validators are
// held to one set of files and cannot drift apart.
const FIXTURES = new URL("../../crates/panel_core/tests/fixtures/booking/", import.meta.url);
const json = (path: string): unknown => JSON.parse(readFileSync(new URL(path, FIXTURES), "utf8"));
const names = (dir: string) => readdirSync(new URL(dir, FIXTURES)).filter((f) => f.endsWith(".json")).sort();
const en = translator("en");

describe("the place booking validator against kitstart's fixtures", () => {
  it("checks Cal.com pages against the fixtures' host list", () => {
    expect(json("rules.json")).toEqual({ calComHosts: [...CAL_COM_HOSTS] });
  });

  it.each(names("valid"))("accepts valid/%s", (file) => {
    expect(Object.fromEntries(checkBookingConfig(json(`valid/${file}`)))).toEqual({});
  });

  it.each(names("invalid"))("refuses invalid/%s", (file) => {
    expect(checkBookingConfig(json(`invalid/${file}`)).size).toBeGreaterThan(0);
  });

  it("has fixtures to read", () => {
    expect(names("valid").length).toBeGreaterThan(10);
    expect(names("invalid").length).toBeGreaterThan(40);
  });
});

describe("the validator's 422 paths", () => {
  // The same cases and paths as `panel_core::booking::tests::config`.
  it.each([
    [{ default: "calendly", providers: {} }, "booking.default"],
    [{ default: "link", providers: {} }, "booking.default"],
    [{ providers: {} }, "booking.default"],
    [{ default: "manual" }, "booking.providers"],
    [{ default: "manual", providers: { manual: { url: "https://x.fr" } } }, "booking.providers.manual"],
    [{ default: "manual", providers: { link: {} } }, "booking.providers.link.url"],
    [{ default: "manual", providers: { link: { url: "http://x.fr" } } }, "booking.providers.link.url"],
    [{ default: "manual", providers: { link: { url: "https://x.fr", label: "x" } } }, "booking.providers.link"],
    [{ default: "manual", providers: { calendly: { url: "https://calendly.com/x" } } }, "booking.providers.calendly"],
    [{ default: "manual", providers: {}, ab: true }, "booking"],
    ["manual", "booking"],
  ])("%j is refused at %s", (body, path) => {
    expect([...checkBookingConfig(body).keys()]).toContain(path);
  });

  it("does not blame the default for its own provider's page", () => {
    const problems = checkBookingConfig({ default: "cal_com", providers: { cal_com: { url: "https://x.fr/a" } } });
    expect(Object.fromEntries(problems)).toEqual({ "booking.providers.cal_com.url": "cal_host" });
  });

  it("names why a page is refused", () => {
    expect(checkBookingUrl("link", "HTTPS://x.fr")).toBe("scheme");
    expect(checkBookingUrl("link", "https://x.fr:443/a")).toBe("port");
    expect(checkBookingUrl("link", "https://10.0.0.1/a")).toBe("ip");
    expect(checkBookingUrl("google_calendar", "https://calendar.google.com/calendar/u/0")).toBe("google");
    expect(checkBookingUrl("cal_com", "https://cal.com/vifnet")).toBe("cal_path");
    expect(checkBookingUrl("cal_com", "https://cal.com/vifnet/menage?x=1")).toBeNull();
  });
});

describe("a place's booking setting", () => {
  const booking = { default: "google_calendar", providers: { google_calendar: { url: "https://calendar.app.google/AbC" }, link: { url: "https://book.example.fr/v" } } };

  it("is a field the form edits, and goes back as it came", () => {
    const s = parse(settingsParser, { phone: "+33612345678", booking });
    expect(s.edited.booking).toEqual(booking);
    expect(s.rest).toEqual({});
    expect(settingsToWire(settingsOf(draftOf(s.edited), s))).toEqual({ phone: "+33612345678", booking: { default: "google_calendar", providers: { link: booking.providers.link, google_calendar: booking.providers.google_calendar } } });
  });

  it("stays untouched in what the form carries when the server would now refuse it", () => {
    const odd = { default: "calendly", providers: {} };
    const s = parse(settingsParser, { booking: odd });
    expect(s.edited.booking).toBeUndefined();
    expect(s.rest).toEqual({ booking: odd });
  });

  it("an untouched form is unchanged, whatever order the server keeps the providers in", () => {
    const s = parse(settingsParser, { booking });
    expect(draftChanged(draftOf(s.edited), s)).toBe(false);
  });

  it("reads in the history in plain words", () => {
    const view = parse(placeSettingsParser, { brand: "a", slug: "b", withdrawn: false, settings: { booking }, updated_at: null, updated_by: null, can_edit: true });
    const config = bookingConfigOf(booking);
    if (config === null) throw new Error("the fixture is a valid booking");
    const text = "Default: Google Calendar; Booking link: https://book.example.fr/v; Google Calendar: https://calendar.app.google/AbC";
    expect(bookingText(config, en)).toBe(text);
    const lines = diffSettings(parse(settingsParser, {}), view.settings, en).map((l) => diffLineText(l, en));
    expect(lines).toEqual([`Booking: set to ${text}`]);
  });
});

describe("the booking block of the form", () => {
  const empty = bookingDraftOf(undefined);

  it("sends nothing while nothing is chosen: the site keeps its own", () => {
    expect(bookingOf(empty)).toBeNull();
    expect(bookingDraftProblems(empty).size).toBe(0);
    expect(editedOf(draftOf({})).booking).toBeUndefined();
  });

  it("asks for a default once a page is typed", () => {
    const d = { ...empty, urls: { ...empty.urls, link: "https://book.example.fr/v" } };
    expect(Object.fromEntries(bookingDraftProblems(d))).toEqual({ "booking.default": "default_choose" });
    expect(draftValid({ ...draftOf({}), booking: d })).toBe(false);
  });

  it("trims what was pasted and leaves an empty page out", () => {
    const d = { default: "cal_com" as const, urls: { link: "  ", google_calendar: "", cal_com: " https://cal.com/vifnet/menage " } };
    expect(bookingOf(d)).toEqual({ default: "cal_com", providers: { cal_com: { url: "https://cal.com/vifnet/menage" } } });
    expect(bookingDraftProblems(d).size).toBe(0);
  });

  it("keys its problems as the server's 422 does, so both land on the same field", () => {
    const d = { default: "google_calendar" as const, urls: { link: "http://x.fr", google_calendar: "", cal_com: "" } };
    expect(Object.fromEntries(bookingDraftProblems(d))).toEqual({ "booking.providers.link.url": "scheme", "booking.default": "default_unset" });
  });

  it("sorts a 422 on booking under the booking field, every key kept", () => {
    const sorted = fieldErrorsOf({ "booking.default": "must be manual or one of the providers set", "booking.providers.google_calendar.url": "must be a Google appointment schedule" });
    expect(sorted.byField.booking).toHaveLength(2);
    expect(sorted.other).toEqual([]);
    expect(sorted.byKey["booking.providers.google_calendar.url"]).toBe("must be a Google appointment schedule");
  });

  it("has words for every problem in both languages", () => {
    const ru = translator("ru");
    for (const p of ["format", "scheme", "fragment", "backslash", "userinfo", "ip", "port", "host", "google", "cal_host", "cal_path", "default_choose", "default_unset"] as const) {
      expect(en(`placeSettings.booking.problem.${p}`)).not.toContain("placeSettings");
      expect(ru(`placeSettings.booking.problem.${p}`)).not.toBe(en(`placeSettings.booking.problem.${p}`));
    }
  });
});
