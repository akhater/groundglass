// Is a newer Groundglass published? Asked at start and every six hours, here in
// the web view: it has HTTPS and follows the phone's network, so the camera's
// Wi-Fi with no internet just fails. A failed check stays silent, keeps what the
// last good one found and tries again in half an hour.
import { useEffect, useState } from "react";
import * as camera from "./camera";

const LATEST_API = "https://api.github.com/repos/akhater/groundglass/releases/latest";
const EVERY_MS = 6 * 3600 * 1000;
const RETRY_MS = 30 * 60 * 1000;
/** A network that swallows requests must not stall the checks. */
const TIMEOUT_MS = 15_000;

/** The latest published release if newer than this app. GitHub's 404: none published yet. */
async function newerRelease() {
  const response = await fetch(LATEST_API, {
    headers: { Accept: "application/vnd.github+json" },
    signal: AbortSignal.timeout(TIMEOUT_MS),
  });
  if (response.status === 404) return null;
  if (!response.ok) throw new Error(`GitHub answered ${response.status}`);
  return camera.newerRelease(await response.text());
}

/** The newer version to announce, or null. */
export function useNewRelease() {
  const [version, setVersion] = useState<string | null>(null);

  useEffect(() => {
    let running = true;
    let timer = 0;
    const check = async () => {
      const next = await newerRelease().catch(() => undefined);
      if (!running) return;
      if (next !== undefined) setVersion(next);
      timer = window.setTimeout(check, next === undefined ? RETRY_MS : EVERY_MS);
    };
    void check();
    return () => {
      running = false;
      clearTimeout(timer);
    };
  }, []);

  return version;
}
