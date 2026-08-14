import { create } from "zustand";
import type {
  ConversionTask,
  ConversionResult,
  TaskStatus,
  ConversionStage,
} from "./types";

interface TaskStore {
  tasks: ConversionTask[];
  currentTask: ConversionTask | null;
  currentResult: ConversionResult | null;
  addTasks: (paths: string[], outputDir: string) => void;
  updateTaskProgress: (
    taskId: string,
    progress: number,
    stage: ConversionStage
  ) => void;
  completeTask: (taskId: string, result: ConversionResult) => void;
  failTask: (taskId: string, error: string) => void;
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

  addTasks: (paths: string[], outputDir: string) =>
    set((state) => {
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
      return { tasks: [...state.tasks, ...newTasks] };
    }),

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
