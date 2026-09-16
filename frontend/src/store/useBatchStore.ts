import { create } from "zustand";
import type {
  BatchTaskDto,
  BatchSummaryDto,
  BatchProgressEvent,
  BatchStatusEvent,
  BatchSummaryEvent,
  TaskStatus,
} from "../types";
import {
  batchListTasks,
  batchGetSummary,
  batchStart,
  batchPauseTask,
  batchResumeTask,
  batchCancelTask,
  batchCancelAll,
  batchRetryFailed,
  batchRetryTask,
  batchClearDone,
  batchSetConcurrency,
  batchEnqueue,
} from "../api/tauriApi";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import type { UnlistenFn } from "@tauri-apps/api/event";
import { showToast } from "../lib/toast";
import { useTaskStore } from "./useTaskStore";
import { useSettingsStore } from "./useSettingsStore";

interface BatchStore {
  tasks: BatchTaskDto[];
  summary: BatchSummaryDto | null;
  loading: boolean;
  concurrency: number;

  setConcurrency: (n: number) => void;
  refreshTasks: () => Promise<void>;
  refreshSummary: () => Promise<void>;
  enqueue: (sourcePath: string, outputPath: string) => Promise<string | null>;
  start: () => Promise<void>;
  pauseTask: (taskId: string) => Promise<void>;
  resumeTask: (taskId: string) => Promise<void>;
  cancelTask: (taskId: string) => Promise<void>;
  cancelAll: () => Promise<void>;
  retryTask: (taskId: string) => Promise<void>;
  retryFailed: () => Promise<void>;
  clearDone: () => Promise<void>;
  listenForEvents: () => Promise<() => void>;
}

export const useBatchStore = create<BatchStore>((set, get) => {
  /** Patch a single task in place (avoids a full list refetch per event). */
  const patchTask = (taskId: string, patch: Partial<BatchTaskDto>) => {
    set((state) => ({
      tasks: state.tasks.map((t) => (t.id === taskId ? { ...t, ...patch } : t)),
    }));
  };

  /** Record a terminal task into the (persisted) conversion history. */
  const recordHistory = async (
    taskId: string,
    status: TaskStatus,
    error: string | null,
  ) => {
    if (status !== "Completed" && status !== "Failed" && status !== "Cancelled") {
      return;
    }
    let task = get().tasks.find((t) => t.id === taskId);
    if (!task) {
      await get().refreshTasks();
      task = get().tasks.find((t) => t.id === taskId);
    }
    if (!task) return;
    useTaskStore.getState().addToHistory({
      id: task.id,
      sourcePath: task.sourcePath,
      outputPath: task.outputPath,
      status,
      error,
      // HistoryEntry timestamps are milliseconds (formatted via `new Date`),
      // while the backend batch DTO reports seconds.
      createdAt: task.createdAt * 1000,
      completedAt: Date.now(),
    });
  };

  return {
    tasks: [],
    summary: null,
    loading: false,
    concurrency: useSettingsStore.getState().concurrency,

    setConcurrency: (n) => {
      const clamped = Math.min(16, Math.max(1, Math.round(n)));
      set({ concurrency: clamped });
      // Persist so the value survives a restart (single source of truth).
      useSettingsStore.getState().setConcurrency(clamped);
      batchSetConcurrency(clamped).catch(() => {});
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

    enqueue: async (sourcePath, outputPath) => {
      try {
        const id = await batchEnqueue(sourcePath, outputPath);
        return id;
      } catch (e) {
        showToast(
          e instanceof Error ? e.message : "Failed to queue file",
          3000,
          "error",
        );
        return null;
      }
    },

    start: async () => {
      // Guard against double-submit (button clicks racing the first call).
      if (get().loading) return;
      set({ loading: true });
      try {
        await batchStart();
        // The backend pushes batch-* events, but refresh here too so the UI
        // updates immediately even if the event listeners aren't registered yet.
        await get().refreshTasks();
        await get().refreshSummary();
      } catch (e) {
        showToast(
          e instanceof Error ? e.message : "Failed to start conversion",
          3000,
          "error",
        );
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

    retryTask: async (taskId) => {
      try {
        await batchRetryTask(taskId);
        await get().refreshTasks();
        await get().refreshSummary();
      } catch (e) {
        showToast(
          e instanceof Error ? e.message : "Failed to retry task",
          3000,
          "error",
        );
      }
    },

    retryFailed: async () => {
      try {
        await batchRetryFailed();
        await get().refreshTasks();
        await get().refreshSummary();
      } catch (e) {
        showToast(
          e instanceof Error ? e.message : "Failed to retry failed tasks",
          3000,
          "error",
        );
      }
    },

    clearDone: async () => {
      try {
        await batchClearDone();
        await get().refreshTasks();
        await get().refreshSummary();
      } catch (e) {
        showToast(
          e instanceof Error ? e.message : "Failed to clear tasks",
          3000,
          "error",
        );
      }
    },

    listenForEvents: async () => {
      const unlisteners: UnlistenFn[] = [];
      try {
        const appWindow = getCurrentWebviewWindow();
        // Incremental updates from the event payload: a full `batch_list_tasks`
        // on every progress tick caused an IPC storm and a full-page re-render.
        const un1 = await appWindow.listen<BatchProgressEvent>(
          "batch-progress",
          (event) => {
            const p = event.payload;
            patchTask(p.taskId, {
              progress: p.progress,
              stage: p.stage,
              elapsedSecs: p.elapsedSecs,
            });
          },
        );
        unlisteners.push(un1);

        const un2 = await appWindow.listen<BatchStatusEvent>(
          "batch-status",
          (event) => {
            const p = event.payload;
            patchTask(p.taskId, {
              status: p.status,
              error: p.error,
              elapsedSecs: p.elapsedSecs,
              ...(p.status === "Completed" ? { progress: 1 } : {}),
            });
            void recordHistory(p.taskId, p.status, p.error);
            void get().refreshSummary();
          },
        );
        unlisteners.push(un2);

        const un3 = await appWindow.listen<BatchSummaryEvent>(
          "batch-summary",
          (event) => {
            set({ summary: event.payload.summary });
          },
        );
        unlisteners.push(un3);
      } catch {
        // Not running in Tauri
      }

      return () => {
        for (const un of unlisteners) {
          try {
            un();
          } catch {
            /* ignore */
          }
        }
      };
    },
  };
});
