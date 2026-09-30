import { defineConfig, globalIgnores } from "eslint/config";
import nextVitals from "eslint-config-next/core-web-vitals";
import nextTs from "eslint-config-next/typescript";

export default defineConfig([
  ...nextVitals,
  ...nextTs,
  {
    // Ported from banking/cabinet/frontend: arbitrary Tailwind values pile up because
    // nothing stops them, so the guard is a rule rather than a convention.
    //
    // Only brackets opening on a digit or `#` are caught: those are the hardcoded sizes
    // and colours. `calc()`, `var()` and `env()` brackets (device insets, the tab bar's
    // height) stay legal, as does `grid-cols-(--name)` for a track with no step on the
    // scale. Bare viewport units are exempt: `max-h-[90dvh]` sizes against the viewport.
    files: ["**/*.ts", "**/*.tsx"],
    rules: {
      "no-restricted-syntax": [
        "error",
        {
          selector: "Literal[value=/-\\[(?!\\d+(?:\\.\\d+)?(?:d|s|l)?v[hw]\\])(?:#|\\d)/]",
          message: "Arbitrary Tailwind value — use a scale step, or a CSS custom property for a genuine one-off.",
        },
        {
          selector: "TemplateElement[value.raw=/-\\[(?!\\d+(?:\\.\\d+)?(?:d|s|l)?v[hw]\\])(?:#|\\d)/]",
          message: "Arbitrary Tailwind value — use a scale step, or a CSS custom property for a genuine one-off.",
        },
      ],
    },
  },
  globalIgnores([".next/**", "out/**", "node_modules/**", "next-env.d.ts"]),
]);
