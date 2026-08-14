export type ThemeMode = "light" | "dark" | "auto";

const STORAGE_KEY = "omnimd_theme";

export function getStoredTheme(): ThemeMode {
  const stored = localStorage.getItem(STORAGE_KEY);
  if (stored === "light" || stored === "dark" || stored === "auto") {
    return stored;
  }
  return "auto";
}

export function storeTheme(theme: ThemeMode): void {
  localStorage.setItem(STORAGE_KEY, theme);
}

function getSystemPrefersDark(): boolean {
  return window.matchMedia("(prefers-color-scheme: dark)").matches;
}

/**
 * Resolve the user's theme choice to the effective light/dark state.
 * "auto" becomes the system preference.
 */
function resolveEffectiveTheme(mode: ThemeMode): "light" | "dark" {
  if (mode === "light") return "light";
  if (mode === "dark") return "dark";
  return getSystemPrefersDark() ? "dark" : "light";
}

/**
 * Apply the theme. Both the data-theme attribute (drives CSS variables)
 * and the dark class (drives Tailwind dark: selectors) are always set
 * to the SAME effective state, so there is never a mismatch.
 */
export function applyTheme(mode?: ThemeMode): void {
  const stored = mode ?? getStoredTheme();
  const html = document.documentElement;
  const effective = resolveEffectiveTheme(stored);

  html.setAttribute("data-theme", effective);
  if (effective === "dark") {
    html.classList.add("dark");
  } else {
    html.classList.remove("dark");
  }
}

export function listenForSystemThemeChange(callback: () => void): () => void {
  const query = window.matchMedia("(prefers-color-scheme: dark)");
  const handler = () => callback();
  query.addEventListener("change", handler);
  return () => query.removeEventListener("change", handler);
}