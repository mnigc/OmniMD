import { useState } from "react";
import {
  Pause,
  Play,
  X,
  Settings2,
  Loader2,
  CheckCircle2,
  XCircle,
  AlertTriangle,
} from "lucide-react";
import { TaskItem } from "../components/TaskItem";
import { useTaskStore } from "../store/useTaskStore";

export function BatchPage() {
  const { tasks, cancelTask, clearCompleted } = useTaskStore();
  const [concurrency, setConcurrency] = useState(4);

  const processingTasks = tasks.filter((t) => t.status === "Processing");
  const completedTasks = tasks.filter((t) => t.status === "Completed");
  const failedTasks = tasks.filter((t) => t.status === "Failed");
  const pendingTasks = tasks.filter((t) => t.status === "Pending");

  return (
    <div className="h-full flex flex-col">
      <div className="border-b border-border px-6 py-3 flex items-center gap-3 shrink-0">
        <h2 className="text-lg font-semibold text-slate-900">Batch Queue</h2>

        <div className="ml-auto flex items-center gap-2">
          <button
            className="flex items-center gap-1.5 px-3 py-1.5 text-sm border border-slate-200 rounded-md hover:bg-slate-100 transition-colors"
          >
            <Pause size={14} />
            Pause
          </button>
          <button
            className="flex items-center gap-1.5 px-3 py-1.5 text-sm border border-slate-200 rounded-md hover:bg-slate-100 transition-colors"
          >
            <Play size={14} />
            Resume
          </button>
          <button
            onClick={() => tasks.forEach((t) => t.status === "Processing" && cancelTask(t.id))}
            className="flex items-center gap-1.5 px-3 py-1.5 text-sm border border-red-200 text-red-600 rounded-md hover:bg-red-50 transition-colors"
          >
            <X size={14} />
            Cancel All
          </button>
          <button
            onClick={clearCompleted}
            className="flex items-center gap-1.5 px-3 py-1.5 text-sm border border-slate-200 rounded-md hover:bg-slate-100 transition-colors"
          >
            <X size={14} />
            Clear Done
          </button>
          <div className="flex items-center gap-2 ml-3 pl-3 border-l border-border">
            <Settings2 size={14} className="text-muted-foreground" />
            <input
              type="number"
              min={1}
              max={16}
              value={concurrency}
              onChange={(e) => setConcurrency(parseInt(e.target.value) || 1)}
              className="w-12 px-2 py-1 text-sm border border-slate-200 rounded-md"
              title="Concurrency"
            />
          </div>
        </div>
      </div>

      <div className="flex-1 overflow-auto p-6">
        <div className="space-y-2 max-w-4xl mx-auto">
          {tasks.map((task) => (
            <TaskItem key={task.id} task={task} />
          ))}

          {tasks.length === 0 && (
            <div className="text-center py-16">
              <Loader2 className="mx-auto mb-4 text-muted-foreground opacity-30" size={48} />
              <p className="text-lg font-medium text-slate-900">
                No tasks in queue
              </p>
              <p className="text-sm text-muted-foreground mt-1">
                Drop files on the Home page to start batch conversion
              </p>
            </div>
          )}
        </div>
      </div>

      <div className="border-t border-border px-6 py-3 bg-slate-50 flex items-center gap-6 shrink-0">
        <div className="flex items-center gap-1.5">
          <Loader2 size={14} className="text-blue-600" />
          <span className="text-sm font-medium">{processingTasks.length}</span>
          <span className="text-xs text-muted-foreground">Processing</span>
        </div>
        <div className="flex items-center gap-1.5">
          <CheckCircle2 size={14} className="text-green-600" />
          <span className="text-sm font-medium">{completedTasks.length}</span>
          <span className="text-xs text-muted-foreground">Completed</span>
        </div>
        <div className="flex items-center gap-1.5">
          <XCircle size={14} className="text-red-600" />
          <span className="text-sm font-medium">{failedTasks.length}</span>
          <span className="text-xs text-muted-foreground">Failed</span>
        </div>
        <div className="flex items-center gap-1.5">
          <AlertTriangle size={14} className="text-slate-400" />
          <span className="text-sm font-medium">{pendingTasks.length}</span>
          <span className="text-xs text-muted-foreground">Pending</span>
        </div>
        <div className="ml-auto text-xs text-muted-foreground">
          Total: {tasks.length}
        </div>
      </div>
    </div>
  );
}
