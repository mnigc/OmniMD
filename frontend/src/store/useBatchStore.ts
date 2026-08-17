import { create } from "zustand";
import type { BatchTaskDto, BatchSummaryDto } from "../types";
import {
  batchListTasks,
  batchGetSummary,
  batchStart,
  batchPauseTask,
  batchResumeTask,
  batchCancelTask,
  batchCancelAll,
  batchRetryFailed,
  batchClearDone,
  batchSetConcurrency,
  batchEnqueue,
} from "../api/tauriApi";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import type { UnlistenFn } from "@tauri-apps/api/event";

interface BatchStore {
  tasks: BatchTaskDto[];
  summary: BatchSummaryDto | null;
  panelOpen: boolean;
  loading: boolean;
  concurrency: number;

  setPanelOpen: (open: boolean) => void;
  setConcurrency: (n: number) => void;
  refreshTasks: () => Promise<void>;
  refreshSummary: () => Promise<void>;
  enqueue: (sourcePath: string, outputPath: string, outputMode?: string, parseQuality?: string) => Promise<string | null>;
  start: () => Promise<void>;
  pauseTask: (taskId: string) => Promise<void>;
  resumeTask: (taskId: string) => Promise<void>;
  cancelTask: (taskId: string) => Promise<void>;
  cancelAll: () => Promise<void>;
  retryFailed: () => Promise<void>;
  clearDone: () => Promise<void>;
  listenForEvents: () => Promise<() => void>;
}

export const useBatchStore = create<BatchStore>((set, get) => ({
  tasks: [],
  summary: null,
  panelOpen: false,
  loading: false,
  concurrency: 3,

  setPanelOpen: (open) => set({ panelOpen: open }),
  setConcurrency: (n) => {
    set({ concurrency: n });
    batchSetConcurrency(n).catch(() => {});
  },

  refreshTasks: async () => {
    try {
      const tasks = await batchListTasks();
      set({ tasks });
    } catch {
      // ignore
    }
  },

  refreshSummary: async () => {
    try {
      const summary = await batchGetSummary();
      set({ summary });
    } catch {
      // ignore
    }
  },

  enqueue: async (sourcePath, outputPath, outputMode, parseQuality) => {
    try {
      const id = await batchEnqueue(sourcePath, outputPath, outputMode as any, parseQuality as any);
      await get().refreshSummary();
      await get().refreshTasks();
      return id;
    } catch (e) {
      return null;
    }
  },

  start: async () => {
    set({ loading: true });
    try {
      await batchStart();
    } catch {
      // ignore
    } finally {
      set({ loading: false });
    }
  },

  pauseTask: async (taskId) => {
    try {
      await batchPauseTask(taskId);
      await get().refreshTasks();
      await get().refreshSummary();
    } catch {
      // ignore
    }
  },

  resumeTask: async (taskId) => {
    try {
      await batchResumeTask(taskId);
      await get().refreshTasks();
      await get().refreshSummary();
    } catch {
      // ignore
    }
  },

  cancelTask: async (taskId) => {
    try {
      await batchCancelTask(taskId);
      await get().refreshTasks();
      await get().refreshSummary();
    } catch {
      // ignore
    }
  },

  cancelAll: async () => {
    try {
      await batchCancelAll();
      await get().refreshTasks();
      await get().refreshSummary();
    } catch {
      // ignore
    }
  },

  retryFailed: async () => {
    try {
      await batchRetryFailed();
      await get().refreshTasks();
      await get().refreshSummary();
    } catch {
      // ignore
    }
  },

  clearDone: async () => {
    try {
      await batchClearDone();
      await get().refreshTasks();
      await get().refreshSummary();
    } catch {
      // ignore
    }
  },

  listenForEvents: async () => {
    const unlisteners: UnlistenFn[] = [];
    try {
      const appWindow = getCurrentWebviewWindow();
      const un1 = await appWindow.listen("batch-progress", () => {
        get().refreshTasks();
      });
      unlisteners.push(un1);

      const un2 = await appWindow.listen("batch-status", () => {
        get().refreshTasks();
        get().refreshSummary();
      });
      unlisteners.push(un2);

      const un3 = await appWindow.listen("batch-summary", () => {
        get().refreshSummary();
      });
      unlisteners.push(un3);
    } catch {
      // Not running in Tauri
    }

    return () => {
      for (const un of unlisteners) {
        try { un(); } catch { /* ignore */ }
      }
    };
  },
}));