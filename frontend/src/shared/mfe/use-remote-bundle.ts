import { useEffect, useState } from "react";

import { containRemoteStyles } from "./contain-remote-styles";

// Loads a remote's ESM bundle once per tag; true once its element is defined, false while
// loading or after a failure (the caller shows its fallback).
export function useRemoteBundle(tag: string, scriptUrl: string): boolean {
  // Keyed by tag: an instance switching remotes is un-ready for the new one at once.
  const [readyTag, setReadyTag] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    const stopContainment = containRemoteStyles(scriptUrl);
    const whenReady = () =>
      customElements
        .whenDefined(tag)
        .then(() => !cancelled && setReadyTag(tag))
        .catch(() => {});

    if (customElements.get(tag) || document.querySelector(`script[data-mfe="${tag}"]`)) {
      void whenReady();
      return () => {
        cancelled = true;
        stopContainment();
      };
    }

    const script = document.createElement("script");
    script.type = "module";
    script.src = scriptUrl;
    script.dataset.mfe = tag;
    // A failed script leaves the DOM, or the guard above would take it for loaded; kept on
    // unmount, so a failure after it still lets a later mount retry.
    const onError = () => script.remove();
    const onLoad = () => {
      script.removeEventListener("error", onError);
      void whenReady();
    };
    script.addEventListener("load", onLoad);
    script.addEventListener("error", onError, { once: true });
    document.head.appendChild(script);
    return () => {
      cancelled = true;
      script.removeEventListener("load", onLoad);
      stopContainment();
    };
  }, [tag, scriptUrl]);

  return readyTag === tag;
}
