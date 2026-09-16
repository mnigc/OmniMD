import { create } from "zustand";
import type {
  ConversionTask,
  ConversionResult,
  HistoryEntry,
} from "../types";

const HISTORY_KEY = "omnimd_history";

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
  history: HistoryEntry[];
  currentTask: ConversionTask | null;
  currentResult: ConversionResult | null;
  previewSource: "live" | "history" | null;

  addToHistory: (entry: HistoryEntry) => void;
  clearHistoryItem: (id: string) => void;
  clearAllHistory: () => void;
  setCurrentTask: (
    task: ConversionTask | null,
    result?: ConversionResult | null,
    source?: "live" | "history" | null
  ) => void;
}

export const useTaskStore = create<TaskStore>((set, get) => ({
  history: loadHistory(),
  currentTask: null,
  currentResult: null,
  previewSource: null,

  addToHistory: (entry) => {
    set((state) => {
      // Deduplicate by id: a task may emit several terminal events, and the
      // same task must not appear in history more than once.
      const history = [entry, ...state.history.filter((e) => e.id !== entry.id)];
      persistHistory(history);
      return { history };
    });
  },

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

  setCurrentTask: (task, result, source) =>
    set({ currentTask: task, currentResult: result ?? null, previewSource: task ? (source ?? "live") : null }),
}));