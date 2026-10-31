// How long one-tap connect leaves the camera alone after Stop.

/**
 * After Stop has dropped Bluetooth and Wi-Fi, a tap 5 s later started the
 * camera's Wi-Fi but the phone couldn't join it; about a minute later it could
 * (R6 Mark III, 2026-11-01). The shortest safe time isn't known.
 */
export const AFTER_STOP_MS = 60_000;

/** How much of that is left at `now`, in ms. `stoppedAt`: when Stop was pressed, null if it hasn't been. */
export function restLeft(stoppedAt: number | null, now: number): number {
  return stoppedAt === null ? 0 : Math.max(0, stoppedAt + AFTER_STOP_MS - now);
}
