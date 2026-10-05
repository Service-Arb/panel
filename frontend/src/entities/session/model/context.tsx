"use client";

import { createContext, useContext, type ReactNode } from "react";

import type { Caller } from "./generated";

const MeContext = createContext<Caller | null>(null);

export function MeProvider({ me, children }: { me: Caller; children: ReactNode }) {
  return <MeContext.Provider value={me}>{children}</MeContext.Provider>;
}

/** The signed-in person; only under the shell, which renders nothing until it knows them. */
export function useMe(): Caller {
  const me = useContext(MeContext);
  if (!me) throw new Error("useMe outside MeProvider");
  return me;
}
