import { create } from "zustand";

const STORAGE_KEY = "omnimd_settings";

interface StoredSettings {
  defaultOutputDir: string;
  concurrency: number;
}

const DEFAULTS: StoredSettings = {
  defaultOutputDir: "",
  concurrency: 3,
};

function loadSettings(): StoredSettings {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (raw) {
      const parsed = JSON.parse(raw);
      return {
        ...DEFAULTS,
        defaultOutputDir: parsed.defaultOutputDir ?? DEFAULTS.defaultOutputDir,
        concurrency: parsed.concurrency ?? DEFAULTS.concurrency,
      };
    }
  } catch {
    // ignore
  }
  return { ...DEFAULTS };
}

function saveSettings(s: StoredSettings) {
  localStorage.setItem(STORAGE_KEY, JSON.stringify(s));
}

interface SettingsStore extends StoredSettings {
  setDefaultOutputDir: (v: string) => void;
  setConcurrency: (v: number) => void;
}

export const useSettingsStore = create<SettingsStore>((set) => {
  const initial = loadSettings();
  const patch = (partial: Partial<StoredSettings>) => {
    set((state) => {
      const next = { ...state, ...partial };
      saveSettings(next);
      return next;
    });
  };

  return {
    ...initial,

    setDefaultOutputDir: (v) => patch({ defaultOutputDir: v }),
    setConcurrency: (v) => patch({ concurrency: v }),
  };
});
