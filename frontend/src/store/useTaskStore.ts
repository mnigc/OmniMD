import { create } from "zustand";
import type {
  ConversionTask,
  ConversionResult,
  TaskStatus,
  ConversionStage,
  OutputMode,
  HistoryEntry,
} from "../types";
import { convertFile, fetchUrl, cancelTask } from "../api/tauriApi";
import { useSettingsStore } from "./useSettingsStore";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import type { UnlistenFn } from "@tauri-apps/api/event";

const HISTORY_KEY = "omnimd_history";
const CONVERSION_CONCURRENCY = 4;

const TERMINAL_STATUSES: TaskStatus[] = ["Completed", "Failed", "Cancelled"];

function generateId(): string {
  return `task-${Date.now()}-${Math.random().toString(36).substr(2, 9)}`;
}

function loadHistory(): HistoryEntry[] {
  try {
    const raw = localStorage.getItem(HISTORY_KEY);
    if (raw) {
      const parsed = JSON.parse(raw);
      if (Array.isArray(parsed)) return parsed as HistoryEntry[];
    }
  } catch {
    // ignore
  }
  return [];
}

function persistHistory(history: HistoryEntry[]) {
  try {
    localStorage.setItem(HISTORY_KEY, JSON.stringify(history));
  } catch {
    // ignore
  }
}

interface TaskStore {
  /* Current batch queued on the Home page (ephemeral, not persisted). */
  sessionTasks: ConversionTask[];
  sessionConverting: boolean;
  /* True while a session cancel is in flight. */
  sessionCancelling: boolean;
  /* Persisted conversion history. */
  history: HistoryEntry[];
  /* Task/result shown in the Convert page editor. */
  currentTask: ConversionTask | null;
  currentResult: ConversionResult | null;
  previewSource: "live" | "history" | null;

  addToSession: (
    paths: string[],
    dirs: string[],
    outputMode: OutputMode
  ) => ConversionTask[];
  removeFromSession: (taskId: string) => void;
  clearSession: () => void;
  updateSessionTaskProgress: (
    taskId: string,
    progress: number,
    stage: ConversionStage
  ) => void;
  updateSessionTaskStatus: (
    taskId: string,
    status: TaskStatus,
    error?: string | null
  ) => void;
  startConversion: () => Promise<void>;
  cancelConversion: () => Promise<void>;
  /* Re-enqueue every Failed task as Pending and restart conversion. */
  retryFailed: () => Promise<void>;

  addToHistory: (entry: HistoryEntry) => void;
  loadHistory: () => void;
  clearHistoryItem: (id: string) => void;
  clearAllHistory: () => void;

  /* Backward-compatible single-file helpers used by the Convert page. */
  addTasks: (paths: string[], outputDir: string) => ConversionTask[];
  finalizeTask: (
    placeholderId: string,
    taskId: string,
    result: ConversionResult,
    error: string | null
  ) => void;
  failTask: (taskId: string, error: string) => void;

  setCurrentTask: (
    task: ConversionTask | null,
    result?: ConversionResult | null,
    source?: "live" | "history" | null
  ) => void;
}

function makeTask(
  path: string,
  outputDir: string,
  outputMode: OutputMode
): ConversionTask {
  const fileName = path.split(/[\\/]/).pop() || "output";
  const outputName = fileName.replace(/\.[^.]+$/, ".md");
  return {
    id: generateId(),
    sourcePath: path,
    outputDir,
    outputPath: `${outputDir}/${outputName}`,
    outputMode,
    status: "Pending",
    progress: 0,
    stage: "Queued",
    error: null,
    createdAt: Date.now(),
    completedAt: null,
  };
}

export const useTaskStore = create<TaskStore>((set, get) => ({
  sessionTasks: [],
  sessionConverting: false,
  sessionCancelling: false,
  history: loadHistory(),
  currentTask: null,
  currentResult: null,
  previewSource: null,

  addToSession: (paths, dirs, outputMode) => {
    const newTasks = paths
      .filter(Boolean)
      .map((path, i) => makeTask(path, dirs[i] || ".", outputMode));
    set((state) => ({
      sessionTasks: [...state.sessionTasks, ...newTasks],
    }));
    return newTasks;
  },

  removeFromSession: (taskId) =>
    set((state) => ({
      sessionTasks: state.sessionTasks.filter((t) => t.id !== taskId),
    })),

  clearSession: () => set({ sessionTasks: [] }),

  updateSessionTaskProgress: (taskId, progress, stage) =>
    set((state) => ({
      sessionTasks: state.sessionTasks.map((t) =>
        t.id === taskId ? { ...t, progress, stage } : t
      ),
    })),

  updateSessionTaskStatus: (taskId, status, error) => {
    const isTerminal = TERMINAL_STATUSES.includes(status);
    set((state) => ({
      sessionTasks: state.sessionTasks.map((t) =>
        t.id === taskId
          ? {
              ...t,
              status,
              error: error ?? t.error,
              completedAt:
                isTerminal && t.completedAt === null ? Date.now() : t.completedAt,
            }
          : t
      ),
    }));
  },

  startConversion: async () => {
    const { sessionTasks, sessionConverting } = get();
    if (sessionConverting) return;

    const pending = sessionTasks.filter((t) => t.status === "Pending");
    if (pending.length === 0) return;

    set({ sessionConverting: true });

    // Listen for progress events from the backend
    const unlisteners: UnlistenFn[] = [];
    try {
      const appWindow = getCurrentWebviewWindow();
      const unlistenProgress = await appWindow.listen<{ taskId: string; progress: number; stage: string }>(
        "task-progress",
        (event) => {
          const { taskId, progress, stage } = event.payload;
          set((state) => ({
            sessionTasks: state.sessionTasks.map((t) =>
              t.id === taskId
                ? { ...t, progress, stage: stage as ConversionStage }
                : t
            ),
          }));
        }
      );
      unlisteners.push(unlistenProgress);

      const unlistenStatus = await appWindow.listen<{ taskId: string; status: string; error?: string }>(
        "task-status",
        (event) => {
          const { taskId, status, error } = event.payload;
          // Only handle terminal status updates from backend (Completed/Failed)
          // Processing status is set locally before the invoke
          set((state) => ({
            sessionTasks: state.sessionTasks.map((t) =>
              t.id === taskId
                ? {
                    ...t,
                    status: status as TaskStatus,
                    error: error ?? t.error,
                    completedAt:
                      (status === "Completed" || status === "Failed") && t.completedAt === null
                        ? Date.now()
                        : t.completedAt,
                  }
                : t
            ),
          }));
        }
      );
      unlisteners.push(unlistenStatus);
    } catch {
      // Not running in Tauri — fall back to no progress events
    }

    const aiReadyOpts = useSettingsStore.getState().buildAiReadyOpts();
    const parseQuality = useSettingsStore.getState().parseQuality;
    const maxInFlight = CONVERSION_CONCURRENCY;
    let nextIndex = 0;

    const worker = async () => {
      while (true) {
        const index = nextIndex;
        nextIndex += 1;
        if (index >= pending.length) break;

        const task = pending[index];
        const live = get().sessionTasks.find((t) => t.id === task.id);
        if (!live || live.status !== "Pending") continue;

        // If a cancel was requested mid-way, don't start new tasks.
        if (get().sessionCancelling) {
          get().updateSessionTaskStatus(task.id, "Cancelled", null);
          continue;
        }

        get().updateSessionTaskStatus(task.id, "Processing", null);

        try {
          const isUrl = task.sourcePath.startsWith("http://") || task.sourcePath.startsWith("https://");
          const result = isUrl
            ? await fetchUrl(
                task.sourcePath,
                task.outputDir,
                task.outputMode,
                aiReadyOpts,
                task.id
              )
            : await convertFile(
                task.sourcePath,
                task.outputDir,
                task.outputMode,
                aiReadyOpts,
                parseQuality,
                task.id
              );
          // The task may have been cancelled/removed while the conversion ran.
          const liveAfter = get().sessionTasks.find((t) => t.id === task.id);
          if (get().sessionCancelling || !liveAfter || liveAfter.status !== "Processing") {
            continue;
          }

          const outputPath = result.outputPath || task.outputPath;
          set((state) => ({
            sessionTasks: state.sessionTasks.map((t) =>
              t.id === task.id
                ? {
                    ...t,
                    status: "Completed",
                    progress: 1,
                    stage: "Saving",
                    outputPath,
                    error: null,
                    completedAt: Date.now(),
                  }
                : t
            ),
          }));
          get().addToHistory({
            id: generateId(),
            sourcePath: task.sourcePath,
            outputPath,
            outputMode: task.outputMode,
            status: "Completed",
            error: null,
            createdAt: task.createdAt,
            completedAt: Date.now(),
          });
        } catch (err: any) {
          const msg = err?.message || String(err);
          const liveAfter = get().sessionTasks.find((t) => t.id === task.id);
          const isCancelled =
            get().sessionCancelling ||
            liveAfter?.status === "Cancelled" ||
            msg.includes("cancelled") ||
            !liveAfter;
          if (isCancelled) {
            get().updateSessionTaskStatus(task.id, "Cancelled", null);
            continue;
          }
          set((state) => ({
            sessionTasks: state.sessionTasks.map((t) =>
              t.id === task.id
                ? {
                    ...t,
                    status: "Failed",
                    error: msg,
                    completedAt: Date.now(),
                  }
                : t
            ),
          }));
          get().addToHistory({
            id: generateId(),
            sourcePath: task.sourcePath,
            outputPath: task.outputPath,
            outputMode: task.outputMode,
            status: "Failed",
            error: msg,
            createdAt: task.createdAt,
            completedAt: Date.now(),
          });
        }
      }
    };

    try {
      const workers = Math.min(maxInFlight, pending.length);
      await Promise.all(Array.from({ length: workers }, () => worker()));
    } finally {
      // Clean up event listeners
      for (const unlisten of unlisteners) {
        try { unlisten(); } catch { /* ignore */ }
      }
      set({ sessionConverting: false, sessionCancelling: false });
    }
  },

  cancelConversion: async () => {
    const { sessionTasks } = get();
    if (!get().sessionConverting) return;

    set({ sessionCancelling: true });

    // Ask the backend to cooperatively stop every in-flight task.
    const inFlight = sessionTasks
      .filter((t) => t.status === "Processing")
      .map((t) => t.id);
    await Promise.allSettled(inFlight.map((id) => cancelTask(id).catch(() => {})));

    // Clear the session list: the user explicitly chose to cancel, so
    // cancelled entries should not linger in the queue.
    set({ sessionTasks: [], sessionConverting: false, sessionCancelling: false });
  },

  retryFailed: async () => {
    if (get().sessionConverting) return;
    const { sessionTasks } = get();
    const failed = sessionTasks.filter((t) => t.status === "Failed");
    if (failed.length === 0) return;

    set((state) => ({
      sessionTasks: state.sessionTasks.map((t) =>
        t.status === "Failed"
          ? { ...t, status: "Pending", progress: 0, stage: "Queued", error: null, completedAt: null }
          : t,
      ),
    }));
    await get().startConversion();
  },

  addToHistory: (entry) => {
    set((state) => {
      const history = [entry, ...state.history];
      persistHistory(history);
      return { history };
    });
  },

  loadHistory: () => set({ history: loadHistory() }),

  clearHistoryItem: (id) => {
    const state = get();
    const history = state.history.filter((e) => e.id !== id);
    persistHistory(history);
    set({ history });
  },

  clearAllHistory: () => {
    persistHistory([]);
    set({ history: [] });
  },

  addTasks: (paths, outputDir) => {
    const outputMode = useSettingsStore.getState().outputMode;
    const dirs = paths.map(() => outputDir);
    return get().addToSession(paths, dirs, outputMode);
  },

  finalizeTask: (placeholderId, taskId, result, error) =>
    set((state) => ({
      sessionTasks: state.sessionTasks.map((t) =>
        t.id !== placeholderId
          ? t
          : {
              ...t,
              id: taskId,
              status: error ? "Failed" : "Completed",
              progress: error ? t.progress : 1,
              outputPath: result.outputPath || t.outputPath,
              error,
              completedAt: Date.now(),
            }
      ),
    })),

  failTask: (taskId, error) =>
    set((state) => ({
      sessionTasks: state.sessionTasks.map((t) =>
        t.id === taskId
          ? { ...t, status: "Failed", error, completedAt: Date.now() }
          : t
      ),
    })),

  setCurrentTask: (task, result, source) =>
    set({ currentTask: task, currentResult: result ?? null, previewSource: task ? (source ?? "live") : null }),
}));
