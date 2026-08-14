import {
  Loader2,
  CheckCircle2,
  XCircle,
  MinusCircle,
  Play,
  Trash2,
  FileText,
  AlertTriangle,
} from "lucide-react";
import type { ConversionTask } from "../types";
import { cn } from "../lib/utils";

interface TaskItemProps {
  task: ConversionTask;
  compact?: boolean;
}

function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

function getFileExtension(path: string): string {
  const parts = path.split("/").pop()?.split("\\").pop() || path;
  const dot = parts.lastIndexOf(".");
  return dot > 0 ? parts.slice(dot + 1).toUpperCase() : "";
}

function getStatusIcon(status: string) {
  switch (status) {
    case "Processing":
      return <Loader2 size={16} className="animate-spin text-blue-600" />;
    case "Completed":
      return <CheckCircle2 size={16} className="text-green-600" />;
    case "Failed":
      return <XCircle size={16} className="text-red-600" />;
    case "Cancelled":
      return <MinusCircle size={16} className="text-slate-400" />;
    default:
      return <AlertTriangle size={16} className="text-slate-400" />;
  }
}

export function TaskItem({ task, compact }: TaskItemProps) {
  const fileName =
    task.sourcePath.split("/").pop()?.split("\\").pop() || task.sourcePath;
  const ext = getFileExtension(task.sourcePath);
  const statusLabel =
    task.status.charAt(0).toUpperCase() + task.status.slice(1).toLowerCase();

  const progressWidth =
    task.status === "Completed"
      ? 100
      : task.status === "Failed"
        ? 0
        : Math.min(Math.round(task.progress * 100), 100);

  const progressBarColor =
    task.status === "Completed"
      ? "bg-green-500"
      : task.status === "Failed"
        ? "bg-red-500"
        : "bg-blue-500";

  return (
    <div
      className={cn(
        "flex items-center gap-3 p-3 rounded-lg border transition-colors",
        compact
          ? "p-2 border-transparent hover:bg-slate-50"
          : "border-slate-200 hover:border-slate-300 bg-white",
        task.status === "Failed" ? "border-red-200 bg-red-50/30" : ""
      )}
    >
      <div className="shrink-0">
        {getStatusIcon(task.status)}
      </div>

      <div className="flex-1 min-w-0">
        <div className="flex items-center gap-2">
          <FileText size={14} className="text-muted-foreground shrink-0" />
          <span className="text-sm font-medium truncate">{fileName}</span>
          {ext && (
            <span className="text-[10px] px-1.5 py-0.5 bg-slate-100 text-slate-600 rounded shrink-0">
              {ext}
            </span>
          )}
        </div>

        {!compact && (
          <div className="mt-1.5 flex items-center gap-2">
            <div className="flex-1 h-1.5 bg-slate-100 rounded-full overflow-hidden">
              <div
                className={`h-full rounded-full transition-all duration-300 ${progressBarColor}`}
                style={{ width: `${progressWidth}%` }}
              />
            </div>
            <span className="text-[10px] text-muted-foreground w-8 text-right">
              {progressWidth}%
            </span>
          </div>
        )}

        {task.error && (
          <p className="text-xs text-red-600 mt-1 truncate">{task.error}</p>
        )}
      </div>

      <div className="flex items-center gap-1 shrink-0">
        <span
          className={cn(
            "text-[10px] px-2 py-0.5 rounded font-medium",
            task.status === "Processing"
              ? "bg-blue-100 text-blue-700"
              : task.status === "Completed"
                ? "bg-green-100 text-green-700"
                : task.status === "Failed"
                  ? "bg-red-100 text-red-700"
                  : task.status === "Cancelled"
                    ? "bg-slate-100 text-slate-600"
                    : "bg-slate-100 text-slate-500"
          )}
        >
          {statusLabel}
        </span>
      </div>

      <div className="flex items-center gap-0.5 shrink-0">
        {task.status === "Pending" && (
          <button
            className="p-1 rounded hover:bg-slate-100 transition-colors"
            title="Start"
          >
            <Play size={14} />
          </button>
        )}
        {(task.status === "Completed" ||
          task.status === "Cancelled" ||
          task.status === "Failed") && (
          <button
            className="p-1 rounded hover:bg-slate-100 transition-colors"
            title="Remove"
          >
            <Trash2 size={14} />
          </button>
        )}
      </div>
    </div>
  );
}
