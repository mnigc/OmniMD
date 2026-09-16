import { createContext, useContext, useState, useMemo, ReactNode } from "react";
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

function readStoredLocale(): Locale {
  try {
    const saved = localStorage.getItem("omnimd_locale");
    if (saved && saved in locales) return saved as Locale;
  } catch {
    // ignore
  }
  return "zh-CN";
}

export function I18nProvider({ children }: { children: ReactNode }) {
  const [locale, setLocaleState] = useState<Locale>(readStoredLocale);

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
