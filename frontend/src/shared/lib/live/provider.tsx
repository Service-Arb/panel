"use client";

import { type ReactNode, createContext, useContext, useEffect, useRef, useState } from "react";

import { liveBus } from "./bus";
import { type Connect, type LiveStatus, createLiveClient } from "./client";
import { LIVE_PATH } from "./protocol";

const StatusContext = createContext<LiveStatus>("closed");

/** `ws:`/`wss:` on the page's own host: the cookie and the `Origin` check need the same origin. */
export function liveUrl(location: Pick<Location, "protocol" | "host">): string {
  return `${location.protocol === "https:" ? "wss:" : "ws:"}//${location.host}${LIVE_PATH}`;
}

const browserConnect: Connect = (url, on) => {
  let ws: WebSocket;
  try {
    ws = new WebSocket(url);
  } catch {
    // A URL the browser refuses outright: report it the way a refused upgrade reports.
    const id = window.setTimeout(() => on.close(1006), 0);
    return { close: () => window.clearTimeout(id) };
  }
  ws.onopen = () => on.open();
  ws.onmessage = (e: MessageEvent) => on.message(e.data);
  ws.onclose = (e: CloseEvent) => on.close(e.code);
  return { close: (code) => ws.close(code) };
};

export interface LiveProviderProps {
  children: ReactNode;
  /** Close 4401: the session ended. */
  onUnauthenticated: () => void;
  /** Close 4403: the person lost access to the panel. */
  onForbidden: () => void;
}

/**
 * The tab's one socket, feeding `liveBus`; mounted once by the shell. Hidden
 * and visible map to the client's pause and resume, `online` to a retry now.
 */
export function LiveProvider({ children, onUnauthenticated, onForbidden }: LiveProviderProps) {
  const [status, setStatus] = useState<LiveStatus>("connecting");
  const handlers = useRef({ onUnauthenticated, onForbidden });
  useEffect(() => {
    handlers.current = { onUnauthenticated, onForbidden };
  });

  useEffect(() => {
    const client = createLiveClient(
      {
        url: liveUrl(window.location),
        connect: browserConnect,
        timers: { set: (fn, ms) => window.setTimeout(fn, ms), clear: (id) => window.clearTimeout(id) },
        random: Math.random,
      },
      {
        status: setStatus,
        changed: (event) => liveBus.emit({ kind: "changed", event }),
        resync: () => liveBus.emit({ kind: "resync" }),
        unauthenticated: () => handlers.current.onUnauthenticated(),
        forbidden: () => handlers.current.onForbidden(),
      },
    );
    const onVisibility = () => (document.visibilityState === "hidden" ? client.pause() : client.resume());
    const onOnline = () => client.nudge();
    document.addEventListener("visibilitychange", onVisibility);
    window.addEventListener("online", onOnline);
    client.start();
    if (document.visibilityState === "hidden") client.pause();
    return () => {
      document.removeEventListener("visibilitychange", onVisibility);
      window.removeEventListener("online", onOnline);
      client.stop();
    };
  }, []);

  return <StatusContext.Provider value={status}>{children}</StatusContext.Provider>;
}

export function useLiveStatus(): LiveStatus {
  return useContext(StatusContext);
}
