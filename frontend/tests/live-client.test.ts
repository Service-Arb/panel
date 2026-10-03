import { describe, expect, it } from "vitest";

import { backoffDelay } from "@/shared/lib/live/backoff";
import { createLiveBus, matchesLive } from "@/shared/lib/live/bus";
import { type LiveStatus, type SocketEvents, createLiveClient } from "@/shared/lib/live/client";
import { type ChangedEvent, parseLiveMessage } from "@/shared/lib/live/protocol";

/** Timers on a clock the test moves. */
function fakeTimers() {
  let now = 0;
  let next = 0;
  const pending = new Map<number, { at: number; ms: number; fn: () => void }>();
  return {
    timers: {
      set(fn: () => void, ms: number) {
        next += 1;
        pending.set(next, { at: now + ms, ms, fn });
        return next;
      },
      clear(id: number) {
        pending.delete(id);
      },
    },
    advance(ms: number) {
      const end = now + ms;
      for (;;) {
        const due = [...pending.entries()].filter(([, t]) => t.at <= end).sort((a, b) => a[1].at - b[1].at)[0];
        if (!due) break;
        pending.delete(due[0]);
        now = due[1].at;
        due[1].fn();
      }
      now = end;
    },
    delays: () => [...pending.values()].map((t) => t.ms),
  };
}

function harness(random = () => 0.5) {
  const clock = fakeTimers();
  const sockets: { url: string; on: SocketEvents; closed: number | null }[] = [];
  const log = { statuses: [] as LiveStatus[], changed: [] as ChangedEvent[], resyncs: 0, unauthenticated: 0, forbidden: 0 };
  const client = createLiveClient(
    {
      url: "ws://panel.test/api/v1/live",
      timers: clock.timers,
      random,
      connect(url, on) {
        const s = { url, on, closed: null as number | null };
        sockets.push(s);
        return { close: (code) => void (s.closed = code ?? 1000) };
      },
    },
    {
      status: (s) => log.statuses.push(s),
      changed: (e) => log.changed.push(e),
      resync: () => void (log.resyncs += 1),
      unauthenticated: () => void (log.unauthenticated += 1),
      forbidden: () => void (log.forbidden += 1),
    },
  );
  const last = () => {
    const s = sockets.at(-1);
    if (!s) throw new Error("no socket yet");
    return s;
  };
  return { client, clock, sockets, log, last, status: () => log.statuses.at(-1) };
}

const changed = (topic: string, extra: Record<string, unknown> = {}) => JSON.stringify({ type: "changed", topic, at: "2026-10-03T10:00:00Z", ...extra });

describe("backoff", () => {
  it("doubles from a second up to the cap, half of each step certain and half jitter", () => {
    expect([0, 1, 2, 3].map((n) => backoffDelay(n, () => 0))).toEqual([500, 1000, 2000, 4000]);
    expect([0, 1, 2, 3].map((n) => backoffDelay(n, () => 1))).toEqual([1000, 2000, 4000, 8000]);
    expect(backoffDelay(20, () => 1)).toBe(30_000);
    expect(backoffDelay(20, () => 0)).toBe(15_000);
  });
});

describe("the live client", () => {
  it("connects once, goes live, and hands on changes", () => {
    const h = harness();
    h.client.start();
    expect(h.sockets).toHaveLength(1);
    expect(h.last().url).toBe("ws://panel.test/api/v1/live");
    h.last().on.open();
    h.last().on.message(JSON.stringify({ type: "hello", at: "2026-10-03T10:00:00Z", user_id: "u" }));
    h.last().on.message(changed("leads", { brand_id: "aquafix", id: "L-1" }));
    expect(h.log.statuses).toEqual(["connecting", "live"]);
    expect(h.log.changed).toEqual([{ topic: "leads", brand_id: "aquafix", id: "L-1", at: "2026-10-03T10:00:00Z" }]);
    // The first open is the page's own first read: nothing was missed yet.
    expect(h.log.resyncs).toBe(0);
  });

  it("ignores frames it cannot read rather than failing", () => {
    const h = harness();
    h.client.start();
    h.last().on.open();
    for (const junk of ["not json", JSON.stringify({ type: "changed", topic: "weather", at: "x" }), JSON.stringify({ topic: "leads" }), 42]) h.last().on.message(junk);
    expect(h.log.changed).toEqual([]);
    expect(h.status()).toBe("live");
  });

  it("reconnects with growing, jittered waits and re-reads everything once back", () => {
    const h = harness(() => 0);
    h.client.start();
    h.last().on.open();
    h.last().on.close(1006);
    expect(h.status()).toBe("reconnecting");
    expect(h.clock.delays()).toEqual([500]);
    h.clock.advance(500);
    expect(h.sockets).toHaveLength(2);
    h.last().on.close(1006);
    expect(h.clock.delays()).toEqual([1000]);
    h.clock.advance(1000);
    h.last().on.open();
    expect(h.status()).toBe("live");
    expect(h.log.resyncs).toBe(1);
    // Back to the first step after a good connection.
    h.last().on.close(1001);
    expect(h.clock.delays()).toEqual([500]);
  });

  it("re-reads everything when the server says resync", () => {
    const h = harness();
    h.client.start();
    h.last().on.open();
    h.last().on.message(JSON.stringify({ type: "resync" }));
    expect(h.log.resyncs).toBe(1);
  });

  it("sends the person to sign-in on 4401 and stops", () => {
    const h = harness();
    h.client.start();
    h.last().on.open();
    h.last().on.close(4401);
    expect(h.log.unauthenticated).toBe(1);
    expect(h.status()).toBe("closed");
    h.clock.advance(120_000);
    expect(h.sockets).toHaveLength(1);
  });

  it("reports lost access on 4403 and does not knock again", () => {
    const h = harness();
    h.client.start();
    h.last().on.close(4403);
    expect(h.log.forbidden).toBe(1);
    h.clock.advance(120_000);
    expect(h.sockets).toHaveLength(1);
  });

  it("falls back to a re-read every minute when the socket never opens, and stops once it does", () => {
    const h = harness(() => 0);
    h.client.start();
    for (let i = 0; i < 3; i++) {
      h.last().on.close(1006); // a 429 or a 404 before the upgrade
      h.clock.advance(4_000);
    }
    expect(h.log.statuses).toContain("offline");
    const before = h.log.resyncs;
    h.clock.advance(60_000);
    expect(h.log.resyncs).toBeGreaterThan(before);
    h.last().on.open();
    expect(h.status()).toBe("live");
    const after = h.log.resyncs;
    h.clock.advance(180_000);
    expect(h.log.resyncs).toBe(after);
  });

  it("ignores a socket it has already replaced", () => {
    const h = harness();
    h.client.start();
    const first = h.last();
    first.on.open();
    first.on.close(1006);
    h.clock.advance(2_000);
    first.on.message(changed("leads"));
    first.on.close(4401);
    expect(h.log.changed).toEqual([]);
    expect(h.log.unauthenticated).toBe(0);
  });

  it("keeps an open socket while hidden, but makes no attempts until visible again", () => {
    const h = harness();
    h.client.start();
    h.last().on.open();
    h.client.pause();
    expect(h.status()).toBe("live");
    h.last().on.message(changed("leads"));
    expect(h.log.changed).toHaveLength(1);
    h.last().on.close(1006);
    expect(h.status()).toBe("paused");
    h.clock.advance(120_000);
    expect(h.sockets).toHaveLength(1);
    h.client.resume();
    expect(h.sockets).toHaveLength(2);
    h.last().on.open();
    expect(h.log.resyncs).toBe(1);
  });

  it("drops a pending attempt when hidden and reconnects at once when shown", () => {
    const h = harness();
    h.client.start();
    h.last().on.close(1006);
    h.client.pause();
    expect(h.last().closed).toBeNull();
    expect(h.clock.delays()).toEqual([]);
    h.client.resume();
    expect(h.sockets).toHaveLength(2);
  });

  it("tries at once when the network comes back", () => {
    const h = harness();
    h.client.start();
    h.last().on.close(1006);
    h.client.nudge();
    expect(h.sockets).toHaveLength(2);
    expect(h.clock.delays()).toEqual([]);
  });

  it("closes its socket on stop", () => {
    const h = harness();
    h.client.start();
    h.last().on.open();
    h.client.stop();
    expect(h.last().closed).toBe(1000);
    expect(h.status()).toBe("closed");
  });
});

describe("the bus", () => {
  const event = (topic: ChangedEvent["topic"]): ChangedEvent => ({ topic, brand_id: "aquafix", id: "x", at: "t" });

  it("hands a change to the readers that follow its topic, and a resync to every reader", () => {
    expect(matchesLive(["leads", "lead"], { kind: "changed", event: event("lead") })).toBe(true);
    expect(matchesLive(["places"], { kind: "changed", event: event("lead") })).toBe(false);
    expect(matchesLive((e) => e.id === "x", { kind: "changed", event: event("sources") })).toBe(true);
    expect(matchesLive([], { kind: "resync" })).toBe(true);
  });

  it("lets a reader leave while being told", () => {
    const bus = createLiveBus();
    const heard: string[] = [];
    const offA = bus.subscribe(() => {
      heard.push("a");
      offA();
    });
    bus.subscribe(() => heard.push("b"));
    bus.emit({ kind: "resync" });
    bus.emit({ kind: "resync" });
    expect(heard).toEqual(["a", "b", "b"]);
  });

  it("reads the wire format", () => {
    expect(parseLiveMessage(changed("places", { brand_id: null }))).toEqual({ type: "changed", topic: "places", brand_id: null, id: null, at: "2026-10-03T10:00:00Z" });
    expect(parseLiveMessage(JSON.stringify({ type: "resync" }))).toEqual({ type: "resync" });
    expect(parseLiveMessage(JSON.stringify({ type: "goodbye" }))).toBeNull();
  });
});
