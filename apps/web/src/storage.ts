/**
 * Per-browser conveniences only (last view, briefing seen). Storage can be
 * blocked or cleared, so every access falls back to in-memory defaults.
 */
export function readStored(key: string): string | null {
  try {
    return window.localStorage.getItem(key)
  } catch {
    return null
  }
}

export function writeStored(key: string, value: string): void {
  try {
    window.localStorage.setItem(key, value)
  } catch {
    // Storage unavailable: the preference lasts for this page load only.
  }
}
