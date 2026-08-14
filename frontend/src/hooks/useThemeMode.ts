import { useCallback, useEffect, useState } from "react";
import {
  type ThemeMode,
  applyTheme,
  getStoredTheme,
  storeTheme,
} from "../lib/theme";

const THEME_CHANGE_EVENT = "omnimd:theme-change";

export function useThemeMode() {
  const [theme, setTheme] = useState<ThemeMode>(getStoredTheme);

  useEffect(() => {
    const handler = () => setTheme(getStoredTheme());
    window.addEventListener(THEME_CHANGE_EVENT, handler);
    return () => window.removeEventListener(THEME_CHANGE_EVENT, handler);
  }, []);

  const setMode = useCallback((mode: ThemeMode) => {
    storeTheme(mode);
    applyTheme(mode);
    setTheme(mode);
    window.dispatchEvent(new Event(THEME_CHANGE_EVENT));
  }, []);

  return { theme, setMode };
}