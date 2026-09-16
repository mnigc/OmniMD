import {
  createContext,
  useContext,
  useEffect,
  useState,
  useMemo,
  ReactNode,
} from "react";
import { zhCN } from "./locales/zh-CN";
import { en } from "./locales/en";

const locales = {
  "zh-CN": zhCN,
  en,
} as const;

export type Locale = keyof typeof locales;
export type TranslationVars = Record<string, string | number>;

interface I18nContextValue {
  t: (path: string, vars?: TranslationVars) => string;
  locale: Locale;
  setLocale: (locale: Locale) => void;
}

const I18nContext = createContext<I18nContextValue | null>(null);

function getNestedValue(obj: Record<string, unknown>, path: string): string {
  const keys = path.split(".");
  let current: unknown = obj;
  for (const key of keys) {
    if (typeof current === "object" && current !== null && key in current) {
      current = (current as Record<string, unknown>)[key];
    } else {
      if (import.meta.env.DEV) {
        console.warn(`[i18n] missing translation key: ${path}`);
      }
      return path;
    }
  }
  return typeof current === "string" ? current : path;
}

function interpolate(value: string, vars?: TranslationVars): string {
  if (!vars) return value;
  let out = value;
  for (const [key, v] of Object.entries(vars)) {
    out = out.split(`{${key}}`).join(String(v));
  }
  return out;
}

function detectDefaultLocale(): Locale {
  try {
    const saved = localStorage.getItem("omnimd_locale");
    if (saved && saved in locales) return saved as Locale;
  } catch {
    // ignore
  }
  // No stored preference: follow the OS/browser language (zh* → zh-CN, else en).
  const navLang = typeof navigator !== "undefined" ? navigator.language : "";
  return navLang.toLowerCase().startsWith("zh") ? "zh-CN" : "en";
}

/** Module-level locale so non-React code (stores, api helpers) can translate. */
let activeLocale: Locale = detectDefaultLocale();

/** Hook-free translator for use outside React components (zustand stores, etc). */
export function translate(path: string, vars?: TranslationVars): string {
  return interpolate(getNestedValue(locales[activeLocale], path), vars);
}

export function I18nProvider({ children }: { children: ReactNode }) {
  const [locale, setLocaleState] = useState<Locale>(activeLocale);

  // Keep <html lang> in sync so screen readers and spellcheck follow the UI
  // language (also covers the initial load).
  useEffect(() => {
    document.documentElement.lang = locale;
  }, [locale]);

  const t = useMemo(
    () => (path: string, vars?: TranslationVars) =>
      interpolate(getNestedValue(locales[locale], path), vars),
    [locale],
  );

  const value = useMemo(
    () => ({
      t,
      locale,
      setLocale: (l: Locale) => {
        activeLocale = l;
        setLocaleState(l);
        try {
          localStorage.setItem("omnimd_locale", l);
        } catch {
          // ignore
        }
      },
    }),
    [t, locale],
  );

  return (
    <I18nContext.Provider value={value}>{children}</I18nContext.Provider>
  );
}

export function useI18n() {
  const ctx = useContext(I18nContext);
  if (!ctx) throw new Error("useI18n must be used within I18nProvider");
  return ctx;
}
