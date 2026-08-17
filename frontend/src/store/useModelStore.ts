import { create } from "zustand";
import type { ModelInfo, CacheInfo, DownloadProgress } from "../types";
import {
  listModels,
  downloadModel,
  cancelModelDownload,
  getCacheInfo,
  clearModelCache,
  setModelSource,
  getModelSource,
  importOfflineModel,
  checkModelUpdate,
} from "../api/tauriApi";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import type { UnlistenFn } from "@tauri-apps/api/event";

interface ModelStore {
  models: ModelInfo[];
  cacheInfo: CacheInfo | null;
  modelSource: string;
  downloading: boolean;
  downloadProgress: Record<string, DownloadProgress>;
  loading: boolean;

  refreshModels: () => Promise<void>;
  refreshCacheInfo: () => Promise<void>;
  refreshModelSource: () => Promise<void>;
  downloadModel: (modelName: string) => Promise<void>;
  cancelDownload: () => Promise<void>;
  clearCache: () => Promise<void>;
  setModelSource: (source: string) => Promise<void>;
  importOffline: (path: string) => Promise<void>;
  checkUpdate: (modelName: string) => Promise<boolean>;
  listenForProgress: () => Promise<() => void>;
}

export const useModelStore = create<ModelStore>((set, get) => ({
  models: [],
  cacheInfo: null,
  modelSource: "auto",
  downloading: false,
  downloadProgress: {},
  loading: false,

  refreshModels: async () => {
    try {
      const models = await listModels();
      set({ models });
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

  downloadModel: async (modelName) => {
    set({ downloading: true });
    try {
      await downloadModel(modelName);
      await get().refreshModels();
      await get().refreshCacheInfo();
    } catch {
      // ignore
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
      await setModelSource(source);
      set({ modelSource: source });
    } catch {
      // ignore
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
            get().refreshCacheInfo();
          }
        }
      );
    } catch {
      // Not running in Tauri
    }

    return () => {
      unlisten?.();
    };
  },
}));