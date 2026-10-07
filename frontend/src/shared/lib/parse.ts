/**
 * Checks for what the API answers. A response is `unknown` until one of these
 * has looked at it: a cast would only promise the shape, and a backend that
 * renamed a field would then fail somewhere far from the cause.
 */

export class ParseError extends Error {
  override name = "ParseError";
}

export type Parser<T> = (value: unknown, path: string) => T;

/** The type a parser yields: what a response is once checked. */
export type Infer<P> = P extends Parser<infer T> ? T : never;

function fail(path: string, want: string, got: unknown): never {
  throw new ParseError(`${path}: expected ${want}, got ${got === null ? "null" : typeof got}`);
}

export const str: Parser<string> = (v, path) => (typeof v === "string" ? v : fail(path, "a string", v));

export const num: Parser<number> = (v, path) =>
  typeof v === "number" && Number.isFinite(v) ? v : fail(path, "a number", v);

/** Integer minor units (cents), never negative: money arrives as a whole number or not at all. */
export const cents: Parser<number> = (v, path) =>
  typeof v === "number" && Number.isSafeInteger(v) && v >= 0 ? v : fail(path, "a whole number of cents, 0 or more", v);

const DAY = /^\d{4}-\d{2}-\d{2}$/;

/** A calendar day, `YYYY-MM-DD`. */
export const isoDay: Parser<string> = (v, path) => (typeof v === "string" && DAY.test(v) ? v : fail(path, "a day, YYYY-MM-DD", v));

export const bool: Parser<boolean> = (v, path) => (typeof v === "boolean" ? v : fail(path, "a boolean", v));

/**
 * Only http(s): the link is put in an `href`, and a `javascript:` one from a
 * misconfigured backend must not become a click away from running.
 */
export const webUrl: Parser<string> = (v, path) => {
  const s = str(v, path);
  let url: URL;
  try {
    url = new URL(s);
  } catch {
    throw new ParseError(`${path}: expected a URL, got ${JSON.stringify(s)}`);
  }
  if (url.protocol !== "https:" && url.protocol !== "http:") throw new ParseError(`${path}: expected an http(s) URL`);
  return s;
};

/** Absent and `null` both read as `null`: the API skips some fields rather than nulling them. */
export function nullable<T>(p: Parser<T>): Parser<T | null> {
  return (v, path) => (v === null || v === undefined ? null : p(v, path));
}

export function arrayOf<T>(p: Parser<T>): Parser<T[]> {
  return (v, path) => (Array.isArray(v) ? v.map((item, i) => p(item, `${path}[${i}]`)) : fail(path, "an array", v));
}

export function oneOf<const L extends readonly string[]>(values: L): Parser<L[number]> {
  return (v, path) =>
    typeof v === "string" && (values as readonly string[]).includes(v)
      ? (v as L[number])
      : fail(path, `one of ${values.join(", ")}`, v);
}

/**
 * A string from `values`, or `fallback` for any other string: for a list the
 * backend may grow before the front ships the new word, so one row does not
 * fail the whole answer. A non-string still fails.
 */
export function oneOfOr<const L extends readonly string[], const F>(values: L, fallback: F): Parser<L[number] | F> {
  return (v, path) => {
    const s = str(v, path);
    return (values as readonly string[]).includes(s) ? (s as L[number]) : fallback;
  };
}

export const record: Parser<Record<string, unknown>> = (v, path) =>
  typeof v === "object" && v !== null && !Array.isArray(v) ? (v as Record<string, unknown>) : fail(path, "an object", v);

/** An object with exactly these keys, each value checked by `p`: a missing key fails. */
export function recordOf<const K extends readonly string[], T>(keys: K, p: Parser<T>): Parser<Record<K[number], T>> {
  return (v, path) => {
    const o = record(v, path);
    return Object.fromEntries(keys.map((k) => [k, p(o[k], `${path}.${k}`)])) as Record<K[number], T>;
  };
}

/** An object of any keys, every value checked by `p`: a map keyed by data, not by the contract. */
export function dictOf<T>(p: Parser<T>): Parser<Record<string, T>> {
  return (v, path) => Object.fromEntries(Object.entries(record(v, path)).map(([k, x]) => [k, p(x, `${path}.${k}`)]));
}

type Shape = Record<string, Parser<unknown>>;
export type Parsed<S extends Shape> = { [K in keyof S]: ReturnType<S[K]> };

/** Unknown extra fields are ignored: the backend may add one before the front reads it. */
export function object<S extends Shape>(shape: S): Parser<Parsed<S>> {
  return (v, path) => {
    const o = record(v, path);
    const out: Record<string, unknown> = {};
    for (const [key, p] of Object.entries(shape)) out[key] = p(o[key], `${path}.${key}`);
    return out as Parsed<S>;
  };
}

export function parse<T>(p: Parser<T>, value: unknown): T {
  return p(value, "$");
}
