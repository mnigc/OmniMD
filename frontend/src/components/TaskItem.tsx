import {
  FileText,
  Play,
  Trash2,
} from "lucide-react";
import type { ConversionTask } from "../types";
import { useI18n } from "../i18n";
import { cn } from "../lib/utils";
import { Badge } from "./ui/badge";
import { Progress } from "./ui/progress";
import { Button } from "./ui/button";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "./ui/tooltip";
import { Spinner } from "./ui/spinner";

interface TaskItemProps {
  task: ConversionTask;
  compact?: boolean;
}

const statusKey: Record<string, string> = {
  Pending: "taskStatus.pending",
  Processing: "taskStatus.processing",
  Completed: "taskStatus.completed",
  Failed: "taskStatus.failed",
  Cancelled: "taskStatus.cancelled",
};

const statusBadgeVariant: Record<
  string,
  "default" | "secondary" | "destructive" | "outline" | "success" | "warning"
> = {
  Pending: "warning",
  Processing: "secondary",
  Completed: "success",
  Failed: "destructive",
  Cancelled: "secondary",
};

function StatusBadge({ status, label }: { status: string; label: string }) {
  if (status === "Processing") {
    return (
      <Badge variant="secondary" className="gap-1.5">
        <Spinner size={10} />
        {label}
      </Badge>
    );
  }
  return <Badge variant={statusBadgeVariant[status] ?? "secondary"}>{label}</Badge>;
}

export function TaskItem({ task, compact }: TaskItemProps) {
  const { t } = useI18n();
  const fileName =
    task.sourcePath.split("/").pop()?.split("\\").pop() || task.sourcePath;
  const ext = fileName?.includes(".")
    ? fileName.slice(fileName.lastIndexOf(".") + 1).toUpperCase()
    : "";
  const statusLabel = t(statusKey[task.status] || "taskStatus.pending");

  const progressWidth =
    task.status === "Completed"
      ? 100
      : task.status === "Failed"
        ? 0
        : Math.min(Math.round(task.progress * 100), 100);

  const progressIndicatorColor =
    task.status === "Completed"
      ? "bg-success"
      : task.status === "Failed"
        ? "bg-destructive"
        : "bg-primary";

  return (
    <div
      className={cn(
        "flex items-center gap-3 rounded-lg border transition-colors",
        compact
          ? "p-2 border-transparent hover:bg-muted"
          : "p-3 bg-card border-border hover:border-primary/40",
        task.status === "Failed" && "border-destructive/30 bg-destructive/5"
      )}
    >
      <FileText
        size={16}
        className="text-muted-foreground shrink-0"
      />

      <div className="flex-1 min-w-0">
        <div className="flex items-center gap-2">
          <span
            className={cn(
              "font-medium truncate",
              compact ? "text-xs" : "text-sm"
            )}
          >
            {fileName}
          </span>
          {ext && (
            <span className="text-xs px-1.5 py-0.5 bg-muted text-muted-foreground rounded shrink-0">
              {ext}
            </span>
          )}
        </div>

        {!compact && (
          <div className="mt-1.5 flex items-center gap-2">
            <Progress
              value={progressWidth}
              indicatorClassName={progressIndicatorColor}
              className="h-1.5"
            />
            <span className="text-xs text-muted-foreground w-8 text-right tabular-nums">
              {progressWidth}%
            </span>
          </div>
        )}

        {task.error && (
          <p className="text-xs text-destructive mt-1 truncate">
            {task.error}
          </p>
        )}
      </div>

      <div className="shrink-0">
        <StatusBadge status={task.status} label={statusLabel} />
      </div>

      <div className="flex items-center gap-0.5 shrink-0">
        {task.status === "Pending" && (
          <Tooltip>
            <TooltipTrigger asChild>
              <Button variant="ghost" size="icon" className="h-7 w-7">
                <Play size={14} />
              </Button>
            </TooltipTrigger>
            <TooltipContent>{t("taskStatus.start")}</TooltipContent>
          </Tooltip>
        )}
        {(task.status === "Completed" ||
          task.status === "Cancelled" ||
          task.status === "Failed") && (
          <Tooltip>
            <TooltipTrigger asChild>
              <Button variant="ghost" size="icon" className="h-7 w-7">
                <Trash2 size={14} />
              </Button>
            </TooltipTrigger>
            <TooltipContent>{t("taskStatus.remove")}</TooltipContent>
          </Tooltip>
        )}
      </div>
    </div>
  );
}