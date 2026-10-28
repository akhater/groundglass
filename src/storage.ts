// Small choices kept between runs, like the camera's address and the last look.

export function remembered(key: string): string | null {
  try {
    return localStorage.getItem(key);
  } catch {
    return null;
  }
}

export function remember(key: string, value: string) {
  try {
    localStorage.setItem(key, value);
  } catch {
    // Only a convenience: the value is chosen again next time.
  }
}

/** A list kept newest first, e.g. the cameras used lately. */
export function rememberedList(key: string): string[] {
  try {
    const list: unknown = JSON.parse(remembered(key) ?? "[]");
    return Array.isArray(list) ? list.filter((item) => typeof item === "string") : [];
  } catch {
    return [];
  }
}

/** Puts `value` first in `list` without repeating it, keeps at most `keep` and remembers the result. */
export function rememberFirst(key: string, list: string[], value: string, keep: number): string[] {
  const next = [value, ...list.filter((item) => item !== value)].slice(0, keep);
  remember(key, JSON.stringify(next));
  return next;
}
