/**
 * What the open card has taken in, to tell the person's own changes from
 * everyone else's. `at` is the lead's latest event as last accepted; after the
 * person acts (`version` moves) the card waits for the latest event to move off
 * `awaiting` — the one on screen when they acted — and takes that as theirs.
 */
export interface Known {
  version: number;
  at: string | null;
  awaiting: string | null;
}

/** The next `Known` given the latest read, or null when it stands. */
export function nextKnown(known: Known, version: number, at: string | null): Known | null {
  if (at === null) return null;
  if (known.at === null) return { version, at, awaiting: null };
  if (version !== known.version) return { version, at: known.at, awaiting: at };
  if (known.awaiting !== null && at !== known.awaiting) return { version, at, awaiting: null };
  return null;
}

/** The lead moved on without the person: someone else, or the site, changed it. */
export function updatedElsewhere(known: Known, version: number, at: string | null): boolean {
  return at !== null && known.at !== null && known.awaiting === null && version === known.version && at !== known.at;
}
