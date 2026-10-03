import { backoffDelay } from "./backoff";
import { CLOSE_FORBIDDEN, CLOSE_UNAUTHENTICATED, type ChangedEvent, parseLiveMessage } from "./protocol";

/**
 * - `connecting` — the first attempt;
 * - `live` — a socket is open;
 * - `reconnecting` — it dropped (or never opened) and the next attempt is scheduled;
 * - `offline` — several attempts in a row never opened: the screens are re-read
 *   on a timer instead, while the attempts go on;
 * - `paused` — the tab is hidden and no socket is open: nothing is attempted;
 * - `closed` — stopped, or the server ended the session for good.
 */
export type LiveStatus = "connecting" | "live" | "reconnecting" | "offline" | "paused" | "closed";

export interface SocketEvents {
  open(): void;
  message(data: unknown): void;
  close(code: number): void;
}

/** Opens a socket that reports through `on`; a refused upgrade (429, 404) is a close without an open. */
export type Connect = (url: string, on: SocketEvents) => { close(code?: number): void };

export interface Timers {
  set(fn: () => void, ms: number): number;
  clear(id: number): void;
}

export interface LiveClientOptions {
  url: string;
  connect: Connect;
  timers: Timers;
  random: () => number;
  /** The fallback re-read while the socket is unavailable. */
  pollMs?: number;
  /** Attempts in a row that never open before the client calls itself offline. */
  failuresBeforeOffline?: number;
}

export interface LiveListener {
  status(status: LiveStatus): void;
  changed(event: ChangedEvent): void;
  /** Re-read everything on screen: events may have been missed. */
  resync(): void;
  unauthenticated(): void;
  forbidden(): void;
}

export interface LiveClient {
  start(): void;
  stop(): void;
  /** The tab went hidden. */
  pause(): void;
  /** The tab is visible again. */
  resume(): void;
  /** The network is back (`online`): try now rather than at the next backoff step. */
  nudge(): void;
}

export const POLL_MS = 60_000;

/**
 * One socket per tab, kept up. Reconnects with exponential backoff and jitter;
 * every open after the first means events may have been missed while it was
 * down, so it asks for a resync. When the socket cannot be had at all, the
 * screens are re-read every `pollMs` instead.
 *
 * A hidden tab keeps an open socket — the tab title counts new leads exactly
 * while the person looks elsewhere — but makes no attempts and no polls while
 * it has none; becoming visible reconnects at once.
 */
export function createLiveClient(options: LiveClientOptions, on: LiveListener): LiveClient {
  const pollMs = options.pollMs ?? POLL_MS;
  const threshold = options.failuresBeforeOffline ?? 3;
  const { timers } = options;

  let status: LiveStatus = "closed";
  let socket: { close(code?: number): void } | null = null;
  // Each socket's handlers check this, so a socket closed on purpose cannot
  // report back into a newer one's state.
  let generation = 0;
  let opens = 0;
  let attempt = 0;
  let failures = 0;
  let hidden = false;
  let ended = false;
  let retry: number | null = null;
  let poll: number | null = null;

  const set = (next: LiveStatus) => {
    if (status === next) return;
    status = next;
    on.status(next);
  };
  const clearRetry = () => {
    if (retry !== null) timers.clear(retry);
    retry = null;
  };
  const stopPoll = () => {
    if (poll !== null) timers.clear(poll);
    poll = null;
  };
  const startPoll = () => {
    if (poll !== null) return;
    const next = () => {
      poll = timers.set(() => {
        on.resync();
        next();
      }, pollMs);
    };
    next();
  };
  const drop = () => {
    generation += 1;
    socket?.close(1000);
    socket = null;
  };
  const end = (final: LiveStatus) => {
    ended = true;
    clearRetry();
    stopPoll();
    drop();
    set(final);
  };

  function connect() {
    clearRetry();
    drop();
    const mine = ++generation;
    let opened = false;
    const ours = () => mine === generation && !ended;
    socket = options.connect(options.url, {
      open() {
        if (!ours()) return;
        opened = true;
        opens += 1;
        const missed = opens > 1 || failures > 0;
        attempt = 0;
        failures = 0;
        stopPoll();
        set("live");
        if (missed) on.resync();
      },
      message(data) {
        if (!ours()) return;
        const msg = parseLiveMessage(data);
        if (msg?.type === "changed") on.changed({ topic: msg.topic, brand_id: msg.brand_id, id: msg.id, at: msg.at });
        else if (msg?.type === "resync") on.resync();
      },
      close(code) {
        if (!ours()) return;
        socket = null;
        if (code === CLOSE_UNAUTHENTICATED || code === CLOSE_FORBIDDEN) {
          end("closed");
          if (code === CLOSE_UNAUTHENTICATED) on.unauthenticated();
          else on.forbidden();
          return;
        }
        if (!opened) failures += 1;
        if (hidden) return set("paused");
        if (failures >= threshold) {
          set("offline");
          startPoll();
        } else set("reconnecting");
        retry = timers.set(connect, backoffDelay(attempt, options.random));
        attempt += 1;
      },
    });
  }

  return {
    start() {
      if (ended || status !== "closed") return;
      set("connecting");
      connect();
    },
    stop() {
      if (!ended) end("closed");
    },
    pause() {
      hidden = true;
      if (ended || status === "live") return;
      clearRetry();
      stopPoll();
      drop();
      set("paused");
    },
    resume() {
      hidden = false;
      if (ended || status !== "paused") return;
      // Unreachable before the pause: the screens have not been re-read since, and
      // the socket may take a while to come back, so read them now.
      if (failures >= threshold) on.resync();
      attempt = 0;
      set(failures >= threshold ? "offline" : "reconnecting");
      if (failures >= threshold) startPoll();
      connect();
    },
    nudge() {
      if (ended || hidden || (status !== "reconnecting" && status !== "offline")) return;
      attempt = 0;
      connect();
    },
  };
}
