import { create } from "zustand";

const STORAGE_KEY = "omnimd_settings";

interface StoredSettings {
  defaultOutputDir: string;
  concurrency: number;
  sidebarDefaultOpen: boolean;
}

const DEFAULTS: StoredSettings = {
  defaultOutputDir: "",
  concurrency: 3,
  sidebarDefaultOpen: true,
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
        sidebarDefaultOpen:
          typeof parsed.sidebarDefaultOpen === "boolean"
            ? parsed.sidebarDefaultOpen
            : DEFAULTS.sidebarDefaultOpen,
      };
    }
  } catch {
    // ignore
  }
  return { ...DEFAULTS };
}

function saveSettings(s: StoredSettings) {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(s));
  } catch {
    // Quota exceeded / storage disabled: never let persistence break state.
  }
}

interface SettingsStore extends StoredSettings {
  setDefaultOutputDir: (v: string) => void;
  setConcurrency: (v: number) => void;
  setSidebarDefaultOpen: (v: boolean) => void;
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
    setSidebarDefaultOpen: (v) => patch({ sidebarDefaultOpen: v }),
  };
});
