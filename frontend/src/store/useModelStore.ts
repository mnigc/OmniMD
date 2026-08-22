import { create } from "zustand";
import type { ModelInfo, CacheInfo, DownloadProgress } from "../types";
import {
  listModels,
  downloadModel as downloadModelApi,
  cancelModelDownload,
  getCacheInfo,
  clearModelCache,
  setModelSource as setModelSourceApi,
  getModelSource,
  importOfflineModel,
  checkModelUpdate,
  isModelDownloaded,
  prepareEnvironment,
} from "../api/tauriApi";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import type { UnlistenFn } from "@tauri-apps/api/event";
import { showToast } from "../lib/toast";

interface ModelStore {
  models: ModelInfo[];
  cacheInfo: CacheInfo | null;
  modelSource: string;
  downloading: boolean;
  downloadProgress: Record<string, DownloadProgress>;
  loading: boolean;
  modelReady: boolean;
  /// Whether the one-time environment preparation (Python + model + MinerU) is
  /// currently running in the background.
  preparing: boolean;
  /// Last preparation error (empty when none / succeeded).
  prepareError: string | null;

  refreshModels: () => Promise<void>;
  refreshCacheInfo: () => Promise<void>;
  refreshModelSource: () => Promise<void>;
  refreshModelReady: () => Promise<void>;
  downloadModel: (modelName: string) => Promise<void>;
  cancelDownload: () => Promise<void>;
  clearCache: () => Promise<void>;
  setModelSource: (source: string) => Promise<void>;
  importOffline: (path: string) => Promise<void>;
  checkUpdate: (modelName: string) => Promise<boolean>;
  /// Kick off automatic environment preparation (no user interaction needed).
  prepare: () => Promise<void>;
  listenForProgress: () => Promise<() => void>;
  listenForEnvPrepare: () => Promise<() => void>;
}

export const useModelStore = create<ModelStore>((set, get) => ({
  models: [],
  cacheInfo: null,
  modelSource: "auto",
  downloading: false,
  downloadProgress: {},
  loading: false,
  modelReady: true,
  preparing: false,
  prepareError: null,

  refreshModels: async () => {
    try {
      const models = await listModels();
      set({ models });
      await get().refreshModelReady();
    } catch {
      // ignore
    }
  },

  refreshCacheInfo: async () => {
    try {
      const cacheInfo = await getCacheInfo();
      set({ cacheInfo });
    } catch {
      // ignore
    }
  },

  refreshModelSource: async () => {
    try {
      const source = await getModelSource();
      set({ modelSource: source });
    } catch {
      // ignore
    }
  },

  refreshModelReady: async () => {
    try {
      const ready = await isModelDownloaded();
      set({ modelReady: ready });
    } catch {
      // ignore
    }
  },

  downloadModel: async (modelName) => {
    set({ downloading: true, downloadProgress: {} });
    try {
      await downloadModelApi(modelName);
      await get().refreshModels();
      await get().refreshCacheInfo();
} catch (e: any) {
      const msg = typeof e === "string" ? e : e?.message ?? String(e ?? "未知错误");
      showToast(msg, 6000);
    } finally {
      set({ downloading: false });
    }
  },

  cancelDownload: async () => {
    try {
      await cancelModelDownload();
      set({ downloading: false });
    } catch {
      // ignore
    }
  },

  clearCache: async () => {
    try {
      await clearModelCache();
      await get().refreshModels();
      await get().refreshCacheInfo();
    } catch {
      // ignore
    }
  },

  setModelSource: async (source) => {
    try {
      await setModelSourceApi(source);
      set({ modelSource: source });
    } catch (e) {
      console.error("setModelSource failed:", e);
    }
  },

  importOffline: async (path) => {
    set({ loading: true });
    try {
      await importOfflineModel(path);
      await get().refreshModels();
      await get().refreshCacheInfo();
    } catch {
      // ignore
    } finally {
      set({ loading: false });
    }
  },

  checkUpdate: async (modelName) => {
    try {
      return await checkModelUpdate(modelName);
    } catch {
      return false;
    }
  },

  listenForProgress: async () => {
    let unlisten: UnlistenFn | null = null;
    try {
      const appWindow = getCurrentWebviewWindow();
      unlisten = await appWindow.listen<DownloadProgress>(
        "model-download-progress",
        (event) => {
          const dp = event.payload;
          if (dp.stage === "cancelled") {
            set({ downloading: false });
            return;
          }
          set((state) => ({
            downloadProgress: { ...state.downloadProgress, [dp.modelName]: dp },
            downloading: dp.progress < 1.0,
          }));
          if (dp.progress >= 1.0) {
            get().refreshModels();
            get().refreshModelReady();
            get().refreshCacheInfo();
          }
        }
      );
    } catch (e) {
      console.error("listenForProgress failed:", e);
    }

    return () => {
      unlisten?.();
    };
  },

  prepare: async () => {
    if (get().preparing) return;
    set({ preparing: true, prepareError: null });
    try {
      await prepareEnvironment();
    } catch (e: any) {
      const msg = typeof e === "string" ? e : e?.message ?? String(e ?? "未知错误");
      set({ preparing: false, prepareError: msg });
    }
  },

  listenForEnvPrepare: async () => {
    let unlisten: UnlistenFn | null = null;
    try {
      const appWindow = getCurrentWebviewWindow();
      unlisten = await appWindow.listen<{
        stage: string;
        progress: number;
        detail: string;
      }>("env-prepare-progress", (event) => {
        const { stage, detail } = event.payload;
        if (stage === "done") {
          set({ preparing: false, prepareError: null, modelReady: true });
        } else if (stage === "error") {
          set({ preparing: false, prepareError: detail });
        } else {
          set({ preparing: true, prepareError: null });
        }
      });
    } catch (e) {
      console.error("listenForEnvPrepare failed:", e);
    }

    return () => {
      unlisten?.();
    };
  },
}));