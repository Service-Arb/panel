export const ROUTES = {
  overview: "/overview",
  leads: "/leads",
  places: "/places",
  pricing: "/pricing",
  experiments: "/experiments",
  sources: "/sources",
  more: "/more",
  signedOut: "/signed-out",
  /** Its own instance behind the backend's auth proxy (spec §6); a full navigation. */
  grafana: "/grafana/",
} as const;
