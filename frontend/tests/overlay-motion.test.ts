import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

import { describe, expect, it } from "vitest";

// The panel's overrides in globals.css patch the kit's Sheet and Dialog motion
// (the scrim flashed dark again on close). They restate the kit's own panel
// durations, so a kit release that changes those must fail here, not drift.
const read = (path: string) => readFileSync(fileURLToPath(new URL(path, import.meta.url)), "utf8");
const css = read("../app/globals.css");
const kit = (name: string) => read(`../node_modules/@evinvest/uikit/dist/generated/${name}.js`);

function kitConst(source: string, name: string): string {
  const m = new RegExp(`const ${name} = "([^"]*)"`).exec(source);
  if (!m?.[1]) throw new Error(`${name} not found in the kit`);
  return m[1];
}

function durationOf(classes: string, state?: "open" | "closed"): number {
  const prefix = state ? `data-\\[state=${state}\\]:` : "(?:^|\\s)";
  const m = new RegExp(`${prefix}duration-(\\d+)`).exec(classes);
  if (!m?.[1]) throw new Error(`no ${state ?? ""} duration in "${classes}"`);
  return Number(m[1]);
}

/** The declarations of the unlayered rule whose selector list is exactly `selector`. */
function rule(selector: string): string {
  const at = css.indexOf(`${selector} {`);
  if (at < 0) throw new Error(`no rule for ${selector}`);
  return css.slice(at, css.indexOf("}", at));
}

const ms = (decls: string) => Number(/animation-duration:\s*(\d+)ms/.exec(decls)?.[1]);

describe("the sheet's scrim", () => {
  const content = kitConst(kit("sheet"), "SHEET_CONTENT");

  it("fades in and out with the panel, not on tw-animate's 150ms default", () => {
    expect(kitConst(kit("sheet"), "SHEET_OVERLAY")).not.toMatch(/duration-/);
    expect(ms(rule('[data-slot="sheet-overlay"][data-state="open"]'))).toBe(durationOf(content, "open"));
    expect(ms(rule('[data-slot="sheet-overlay"][data-state="closed"]'))).toBe(durationOf(content, "closed"));
  });
});

describe("the dialogs' scrim", () => {
  it("lasts as long as the dialog", () => {
    const decls = rule('[data-slot="dialog-overlay"],\n[data-slot="alert-dialog-overlay"]');
    expect(ms(decls)).toBe(durationOf(kitConst(kit("dialog"), "DIALOG_CONTENT")));
    expect(ms(decls)).toBe(durationOf(kitConst(kit("alert-dialog"), "ALERT_DIALOG_CONTENT")));
  });
});

describe("a closing overlay", () => {
  it("holds its last frame until it unmounts instead of snapping back", () => {
    const at = css.indexOf("animation-fill-mode: forwards");
    const selectors = css.slice(css.lastIndexOf("}", at) + 1, css.lastIndexOf("{", at));
    for (const slot of ["sheet-overlay", "sheet-content", "dialog-overlay", "dialog-content", "alert-dialog-overlay", "alert-dialog-content"]) {
      expect(selectors).toContain(`[data-slot="${slot}"][data-state="closed"]`);
    }
  });
});
