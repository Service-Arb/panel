import { http, ignoreBody } from "@/shared/api";

import { type AddedSource, type Source, type SourceKind, addedSourceParser, sourcesParser } from "../model/source";

export async function fetchSources(): Promise<Source[]> {
  return (await http.get("/api/v1/sources", sourcesParser)).sources;
}

export interface NewSource {
  key_id: string;
  kind: SourceKind;
  brands: string[];
}

/** The answer is the only place the secret ever is: the caller shows it once. */
export function addSource(source: NewSource): Promise<AddedSource> {
  return http.send("POST", "/api/v1/sources", source, addedSourceParser);
}

export async function revokeSource(keyId: string): Promise<void> {
  await http.send("DELETE", `/api/v1/sources/${encodeURIComponent(keyId)}`, undefined, ignoreBody);
}
