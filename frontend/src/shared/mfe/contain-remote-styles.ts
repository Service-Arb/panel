// A light-DOM remote styles itself with a stylesheet it injects into <head>; loading last, its
// utilities would beat the panel's. Every sheet served from the remote's bundle directory is
// moved into the `mfe` layer, declared below all of the panel's (app/globals.css). Keyed on the
// path: the remote is served from this origin.
export function containRemoteStyles(scriptUrl: string): () => void {
  if (typeof document === "undefined") return () => {};
  const base = new URL(scriptUrl, window.location.href);
  const assetDir = base.href.slice(0, base.href.lastIndexOf("/") + 1);

  const demote = (link: HTMLLinkElement) => {
    const style = document.createElement("style");
    style.dataset.mfe = "";
    style.textContent = `@import url(${JSON.stringify(link.href)}) layer(mfe);`;
    link.replaceWith(style);
  };

  const scan = () => {
    document.head.querySelectorAll<HTMLLinkElement>('link[rel="stylesheet"]').forEach((link) => {
      if (link.href.startsWith(assetDir)) demote(link);
    });
  };

  scan();
  const observer = new MutationObserver(scan);
  observer.observe(document.head, { childList: true });
  return () => observer.disconnect();
}
