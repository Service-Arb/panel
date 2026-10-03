"use client";

import { createContext, useContext } from "react";

/** What the shell's nav marks, by route: a count, or "changed". Read by screens that link onward (More). */
export type NavMarks = Readonly<Record<string, number | boolean>>;

const NavMarksContext = createContext<NavMarks>({});

export const NavMarksProvider = NavMarksContext.Provider;

export function useNavMark(route: string): number | boolean {
  return useContext(NavMarksContext)[route] ?? false;
}
