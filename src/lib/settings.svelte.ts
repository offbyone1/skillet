// App settings — persisted to localStorage, applied as data-* attributes on
// <html> so the rest of the styling is plain CSS. Shared rune state so the
// sidebar trigger (+page) and the pop-out render (+layout) stay in sync.

export type Theme = "dark" | "light";

type Settings = {
  theme: Theme;
  reduceMotion: boolean;
  showBackdrop: boolean;
  autoUpdate: boolean;
};

const KEY = "skillet.settings";

function load(): Partial<Settings> {
  if (typeof localStorage === "undefined") return {};
  try {
    return JSON.parse(localStorage.getItem(KEY) ?? "{}");
  } catch {
    return {};
  }
}

const saved = load();

export const settings = $state<Settings>({
  theme: saved.theme ?? "dark",
  reduceMotion: saved.reduceMotion ?? false,
  showBackdrop: saved.showBackdrop ?? true,
  autoUpdate: saved.autoUpdate ?? true,
});

// Pop-out open state + the grow-from origin offset (deck-style animation).
export const ui = $state<{ open: boolean; origin: { x: number; y: number } }>({
  open: false,
  origin: { x: 0, y: 0 },
});

export function applySettings() {
  if (typeof document === "undefined") return;
  const el = document.documentElement;
  el.dataset.theme = settings.theme;
  el.dataset.motion = settings.reduceMotion ? "reduced" : "full";
  el.dataset.backdrop = settings.showBackdrop ? "on" : "off";
}

export function persist() {
  if (typeof localStorage === "undefined") return;
  try {
    localStorage.setItem(KEY, JSON.stringify(settings));
  } catch {
    /* ignore quota / privacy-mode errors */
  }
}

// apply immediately on first import so the theme is right before first paint
applySettings();
