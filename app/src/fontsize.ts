/** Text size of the terminals (points) and the sidebar (percent), set in 外观… and with Cmd+= / Cmd+- / Cmd+0 (the
 *  terminals). Kept in localStorage like the theme; changes apply live. */

export interface FontSizes {
  terminal: number;
  sidebar: number;
}

export const DEFAULT_SIZES: FontSizes = { terminal: 14, sidebar: 100 };
export const TERMINAL_RANGE = { min: 10, max: 24, step: 1 };
export const SIDEBAR_RANGE = { min: 80, max: 150, step: 10 };

const KEY = "shifu.fontSizes";
const listeners = new Set<(s: FontSizes) => void>();

const clamp = (v: unknown, range: { min: number; max: number }, fallback: number): number =>
  typeof v === "number" && Number.isFinite(v) ? Math.min(range.max, Math.max(range.min, Math.round(v))) : fallback;

/** Whatever was stored, as sizes in range; anything unreadable falls back to the defaults. */
export function readSizes(stored: string | null): FontSizes {
  let v: Partial<FontSizes> = {};
  try {
    const parsed: unknown = JSON.parse(stored ?? "{}");
    if (typeof parsed === "object" && parsed !== null) v = parsed as Partial<FontSizes>;
  } catch {
    // keep the defaults
  }
  return {
    terminal: clamp(v.terminal, TERMINAL_RANGE, DEFAULT_SIZES.terminal),
    sidebar: clamp(v.sidebar, SIDEBAR_RANGE, DEFAULT_SIZES.sidebar),
  };
}

/** localStorage is missing outside the window (tests). */
const stored = (): string | null => (typeof localStorage === "undefined" ? null : localStorage.getItem(KEY));

let current = readSizes(stored());

export const getFontSizes = (): FontSizes => current;

function paint(s: FontSizes): void {
  document.documentElement.style.setProperty("--sidebar-zoom", String(s.sidebar / 100));
}

export function applyStoredFontSizes(): void {
  paint(current);
}

export function setFontSizes(next: Partial<FontSizes>): void {
  current = readSizes(JSON.stringify({ ...current, ...next }));
  localStorage.setItem(KEY, JSON.stringify(current));
  paint(current);
  listeners.forEach((f) => f(current));
}

/** One step bigger (+1) or smaller (-1) for the terminals; 0 goes back to the default. */
export function stepTerminal(direction: -1 | 0 | 1): void {
  const terminal = direction === 0 ? DEFAULT_SIZES.terminal : current.terminal + direction * TERMINAL_RANGE.step;
  setFontSizes({ terminal });
}

export function onFontSizesChange(f: (s: FontSizes) => void): () => void {
  listeners.add(f);
  return () => listeners.delete(f);
}
