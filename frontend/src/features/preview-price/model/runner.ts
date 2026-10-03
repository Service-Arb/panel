export type Settled<R> = { ok: true; value: R } | { ok: false; error: unknown };

export interface Timers {
  setTimeout: (fn: () => void, ms: number) => unknown;
  clearTimeout: (id: unknown) => void;
}

export interface Runner<P> {
  /** The newest thing to compute; anything asked before it and not yet answered is dropped. */
  request: (key: string, payload: P) => void;
  /** Drops what is waiting or under way; a later `request` works as before (a remount reuses the runner). */
  cancel: () => void;
}

const realTimers: Timers = {
  setTimeout: (fn, ms) => globalThis.setTimeout(fn, ms),
  clearTimeout: (id) => globalThis.clearTimeout(id as ReturnType<typeof globalThis.setTimeout>),
};

/**
 * Typing in the editor asks for a price on every keystroke; this asks the
 * server once the typing has paused for `delayMs`, and only the answer to the
 * latest request is ever reported — a slow answer to an older draft must not
 * land over a newer one.
 */
export function createRunner<P, R>(opts: { delayMs: number; run: (payload: P) => Promise<R>; onSettled: (key: string, result: Settled<R>) => void; timers?: Timers }): Runner<P> {
  const timers = opts.timers ?? realTimers;
  let generation = 0;
  let timer: unknown = null;
  return {
    request(key, payload) {
      generation += 1;
      const mine = generation;
      if (timer !== null) timers.clearTimeout(timer);
      timer = timers.setTimeout(() => {
        timer = null;
        const report = (result: Settled<R>) => {
          if (mine === generation) opts.onSettled(key, result);
        };
        opts.run(payload).then(
          (value) => report({ ok: true, value }),
          (error: unknown) => report({ ok: false, error }),
        );
      }, opts.delayMs);
    },
    cancel() {
      generation += 1;
      if (timer !== null) timers.clearTimeout(timer);
      timer = null;
    },
  };
}
