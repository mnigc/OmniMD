import { create } from "zustand";
import type {
  ConversionTask,
  ConversionResult,
  TaskStatus,
  ConversionStage,
} from "../types";

interface TaskStore {
  tasks: ConversionTask[];
  currentTask: ConversionTask | null;
  currentResult: ConversionResult | null;
  addTasks: (paths: string[], outputDir: string) => ConversionTask[];
  updateTaskProgress: (
    taskId: string,
    progress: number,
    stage: ConversionStage
  ) => void;
  completeTask: (taskId: string, result: ConversionResult) => void;
  failTask: (taskId: string, error: string) => void;
  finalizeTask: (
    placeholderId: string,
    taskId: string,
    result: ConversionResult,
    error: string | null
  ) => void;
  cancelTask: (taskId: string) => void;
  clearCompleted: () => void;
  setCurrentTask: (task: ConversionTask | null, result?: ConversionResult) => void;
  selectTask: (taskId: string) => void;
}

function generateId(): string {
  return `task-${Date.now()}-${Math.random().toString(36).substr(2, 9)}`;
}

export const useTaskStore = create<TaskStore>((set) => ({
  tasks: [],
  currentTask: null,
  currentResult: null,

  addTasks: (paths: string[], outputDir: string) => {
    const newTasks: ConversionTask[] = paths.map((path) => {
      const outputName = path.split("/").pop()?.replace(/\.[^.]+$/, ".md");
      return {
        id: generateId(),
        sourcePath: path,
        outputPath: `${outputDir}/${outputName}`,
        status: "Pending" as TaskStatus,
        progress: 0,
        stage: "DetectingFormat" as ConversionStage,
        error: null,
        createdAt: Date.now(),
        completedAt: null,
      };
    });
    set((state) => ({ tasks: [...state.tasks, ...newTasks] }));
    return newTasks;
  },

  updateTaskProgress: (taskId, progress, stage) =>
    set((state) => ({
      tasks: state.tasks.map((t) =>
        t.id === taskId ? { ...t, progress, stage } : t
      ),
    })),

  completeTask: (taskId, result) =>
    set((state) => ({
      tasks: state.tasks.map((t) =>
        t.id === taskId
          ? {
              ...t,
              status: "Completed" as TaskStatus,
              progress: 1,
              completedAt: Date.now(),
            }
          : t
      ),
    })),

  failTask: (taskId, error) =>
    set((state) => ({
      tasks: state.tasks.map((t) =>
        t.id === taskId
          ? {
              ...t,
              status: "Failed" as TaskStatus,
              error,
              completedAt: Date.now(),
            }
          : t
      ),
    })),

  finalizeTask: (placeholderId, taskId, result, error) =>
    set((state) => ({
      tasks: state.tasks.map((t) =>
        t.id !== placeholderId
          ? t
          : {
              ...t,
              id: taskId,
              status: error ? ("Failed" as TaskStatus) : ("Completed" as TaskStatus),
              progress: error ? t.progress : 1,
              error,
              completedAt: Date.now(),
            }
      ),
    })),

  cancelTask: (taskId) =>
    set((state) => ({
      tasks: state.tasks.map((t) =>
        t.id === taskId
          ? { ...t, status: "Cancelled" as TaskStatus }
          : t
      ),
    })),

  clearCompleted: () =>
    set((state) => ({
      tasks: state.tasks.filter(
        (t) => t.status !== "Completed" && t.status !== "Cancelled"
      ),
    })),

  setCurrentTask: (task, result) =>
    set({ currentTask: task, currentResult: result ?? null }),

  selectTask: (taskId) =>
    set((state) => {
      const task = state.tasks.find((t) => t.id === taskId);
      const result = null;
      return { currentTask: task ?? null, currentResult: result };
    }),
}));
