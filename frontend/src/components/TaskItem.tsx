import {
  FileText,
  FolderOpen,
  Link,
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
  showActions?: boolean;
  onStart?: (taskId: string) => void;
  onRemove?: (taskId: string) => void;
  onOpenFile?: (taskId: string) => void;
  onOpenFolder?: (taskId: string) => void;
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

export function TaskItem({
  task,
  compact,
  showActions = true,
  onStart,
  onRemove,
  onOpenFile,
  onOpenFolder,
}: TaskItemProps) {
  const { t } = useI18n();
  const isUrl = task.sourcePath.startsWith("http://") || task.sourcePath.startsWith("https://");
  const fileName = isUrl
    ? task.sourcePath
    : task.sourcePath.split("/").pop()?.split("\\").pop() || task.sourcePath;
  const ext = isUrl
    ? ""
    : fileName?.includes(".")
      ? fileName.slice(fileName.lastIndexOf(".") + 1).toUpperCase()
      : "";
  const statusLabel = t(statusKey[task.status] || "taskStatus.pending");
  const modeLabel = task.outputMode ? t(`outputMode.${task.outputMode}`) : "";

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
      {isUrl ? (
        <Link
          size={16}
          className="text-muted-foreground shrink-0"
        />
      ) : (
        <FileText
          size={16}
          className="text-muted-foreground shrink-0"
        />
      )}

      <div className="flex-1 min-w-0">
        <div className="flex items-center gap-2">
            <Tooltip>
              <TooltipTrigger asChild>
                <span
                  className={cn(
                    "font-medium truncate cursor-help",
                    compact ? "text-xs" : "text-sm"
                  )}
                >
                  {fileName}
                </span>
              </TooltipTrigger>
              <TooltipContent side="bottom" align="start" className="max-w-xs text-xs">
                {fileName}
              </TooltipContent>
            </Tooltip>
            {ext && (
              <span className="text-xs px-1.5 py-0.5 bg-muted text-muted-foreground rounded shrink-0">
                {ext}
              </span>
            )}
            {modeLabel && (
              <span className="text-[10px] px-1.5 py-0.5 bg-primary/10 text-primary rounded shrink-0">
                {modeLabel}
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
          <Tooltip>
            <TooltipTrigger asChild>
              <p className="text-xs text-destructive mt-1 line-clamp-3 break-words cursor-help">
                {task.error}
              </p>
            </TooltipTrigger>
            <TooltipContent
              side="bottom"
              align="start"
              className="max-w-xs p-2 text-xs"
            >
              <p className="whitespace-pre-wrap break-words">{task.error}</p>
            </TooltipContent>
          </Tooltip>
        )}
      </div>

      <div className="shrink-0">
        <StatusBadge status={task.status} label={statusLabel} />
      </div>

      {showActions && (
        <div className="flex items-center gap-0.5 shrink-0">
          {task.status === "Pending" && onStart && (
            <Tooltip>
              <TooltipTrigger asChild>
                <Button
                  variant="ghost"
                  size="icon"
                  className="h-7 w-7"
                  onClick={() => onStart(task.id)}
                >
                  <Play size={14} />
                </Button>
              </TooltipTrigger>
              <TooltipContent>{t("taskStatus.start")}</TooltipContent>
            </Tooltip>
          )}
          {task.status === "Completed" && onOpenFile && (
            <Tooltip>
              <TooltipTrigger asChild>
                <Button
                  variant="ghost"
                  size="icon"
                  className="h-7 w-7"
                  onClick={() => onOpenFile(task.id)}
                >
                  <FileText size={14} />
                </Button>
              </TooltipTrigger>
              <TooltipContent>{t("history.openFile")}</TooltipContent>
            </Tooltip>
          )}
          {task.status === "Completed" && onOpenFolder && (
            <Tooltip>
              <TooltipTrigger asChild>
                <Button
                  variant="ghost"
                  size="icon"
                  className="h-7 w-7"
                  onClick={() => onOpenFolder(task.id)}
                >
                  <FolderOpen size={14} />
                </Button>
              </TooltipTrigger>
              <TooltipContent>{t("history.openFolder")}</TooltipContent>
            </Tooltip>
          )}
          {(task.status === "Completed" ||
            task.status === "Cancelled" ||
            task.status === "Failed") &&
            onRemove && (
              <Tooltip>
                <TooltipTrigger asChild>
                  <Button
                    variant="ghost"
                    size="icon"
                    className="h-7 w-7"
                    onClick={() => onRemove(task.id)}
                  >
                    <Trash2 size={14} />
                  </Button>
                </TooltipTrigger>
                <TooltipContent>{t("history.delete")}</TooltipContent>
              </Tooltip>
            )}
        </div>
      )}
    </div>
  );
}