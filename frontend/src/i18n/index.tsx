import { createContext, useContext, useState, useMemo, ReactNode } from "react";
import { zhCN } from "./locales/zh-CN";
import { en } from "./locales/en";

const locales = {
  "zh-CN": zhCN,
  en,
} as const;

export type Locale = keyof typeof locales;
type TranslationType = typeof zhCN;

interface I18nContextValue {
  t: (path: string) => string;
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
      return path;
    }
  }
  return typeof current === "string" ? current : path;
}

export function I18nProvider({ children }: { children: ReactNode }) {
  const saved = localStorage.getItem("omnimd_locale");
  const [locale, setLocale] = useState<Locale>(
    (saved as Locale) && locales[saved as Locale] ? (saved as Locale) : "zh-CN"
  );

  const t = useMemo(
    () => (path: string) => getNestedValue(locales[locale], path),
    [locale]
  );

  const value = useMemo(
    () => ({
      t,
      locale,
      setLocale: (l: Locale) => {
        setLocale(l);
        localStorage.setItem("omnimd_locale", l);
      },
    }),
    [t, locale]
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