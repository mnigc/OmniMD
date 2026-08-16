import { create } from "zustand";
import type { AiReadyOpts, OutputMode } from "../types";

const STORAGE_KEY = "omnimd_settings";

export type OcrMode = "off" | "auto" | "always";

interface StoredSettings {
  ocrMode: OcrMode;
  outputMode: OutputMode;
  defaultOutputDir: string;
  recursive: boolean;
  keepStructure: boolean;
  aiEnabled: boolean;
  aiReadyToc: boolean;
  aiReadyMeta: boolean;
}

const DEFAULTS: StoredSettings = {
  ocrMode: "auto",
  outputMode: "aiReady",
  defaultOutputDir: "",
  recursive: true,
  keepStructure: true,
  aiEnabled: false,
  aiReadyToc: false,
  aiReadyMeta: false,
};

function loadSettings(): StoredSettings {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (raw) {
      const parsed = JSON.parse(raw);
      return {
        ...DEFAULTS,
        ocrMode: parsed.ocrMode ?? (parsed.ocrEnabled === true ? "always" : "auto"),
        outputMode: parsed.outputMode ?? DEFAULTS.outputMode,
        defaultOutputDir: parsed.defaultOutputDir ?? DEFAULTS.defaultOutputDir,
        recursive: parsed.recursive ?? DEFAULTS.recursive,
        keepStructure: parsed.keepStructure ?? DEFAULTS.keepStructure,
        aiEnabled: parsed.aiEnabled ?? DEFAULTS.aiEnabled,
        aiReadyToc: parsed.aiReadyToc ?? DEFAULTS.aiReadyToc,
        aiReadyMeta: parsed.aiReadyMeta ?? DEFAULTS.aiReadyMeta,
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
  setOcrMode: (v: OcrMode) => void;
  setOutputMode: (m: OutputMode) => void;
  setDefaultOutputDir: (v: string) => void;
  setRecursive: (v: boolean) => void;
  setKeepStructure: (v: boolean) => void;
  setAiEnabled: (v: boolean) => void;
  setAiReadyToc: (v: boolean) => void;
  setAiReadyMeta: (v: boolean) => void;
  /** Build the AiReadyOpts payload for the backend, gated by the master AI
   *  toggle so the optional TOC/metadata enhancements only apply when the AI
   *  feature group is enabled. Returns undefined when there is nothing to do
   *  (all flags false), letting the backend keep its defaults. */
  buildAiReadyOpts: () => AiReadyOpts | undefined;
}

export const useSettingsStore = create<SettingsStore>((set, get) => {
  const initial = loadSettings();
  const patch = (partial: Partial<StoredSettings>) => {
    set((state) => {
      const next = { ...state, ...partial };
      saveSettings(next);
      return partial;
    });
  };

  return {
    ...initial,

    setOcrMode: (v) => patch({ ocrMode: v }),
    setOutputMode: (m) => patch({ outputMode: m }),
    setDefaultOutputDir: (v) => patch({ defaultOutputDir: v }),
    setRecursive: (v) => patch({ recursive: v }),
    setKeepStructure: (v) => patch({ keepStructure: v }),
    setAiEnabled: (v) => patch({ aiEnabled: v }),
    setAiReadyToc: (v) => patch({ aiReadyToc: v }),
    setAiReadyMeta: (v) => patch({ aiReadyMeta: v }),

    buildAiReadyOpts: () => {
      const s = get();
      const genToc = s.aiEnabled && s.aiReadyToc;
      const genMeta = s.aiEnabled && s.aiReadyMeta;
      if (!genToc && !genMeta) return undefined;
      return { genToc, genMeta };
    },
  };
});