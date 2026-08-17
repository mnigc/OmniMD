import { FileText, Link, Pause, Play, X, RotateCcw } from "lucide-react";
import type { BatchTaskDto } from "../types";
import { useI18n } from "../i18n";
import { cn } from "../lib/utils";
import { Badge } from "./ui/badge";
import { Progress } from "./ui/progress";
import { Button } from "./ui/button";
import { Tooltip, TooltipContent, TooltipTrigger } from "./ui/tooltip";
import { Spinner } from "./ui/spinner";
import { useBatchStore } from "../store/useBatchStore";

interface BatchTaskItemProps {
  task: BatchTaskDto;
}

const statusBadgeVariant: Record<string, "default" | "secondary" | "destructive" | "outline" | "success" | "warning"> = {
  Pending: "warning",
  Processing: "secondary",
  Completed: "success",
  Failed: "destructive",
  Cancelled: "secondary",
  Paused: "outline",
};

const statusKey: Record<string, string> = {
  Pending: "taskStatus.pending",
  Processing: "taskStatus.processing",
  Completed: "taskStatus.completed",
  Failed: "taskStatus.failed",
  Cancelled: "taskStatus.cancelled",
  Paused: "batch.pause",
};

export function BatchTaskItem({ task }: BatchTaskItemProps) {
  const { t } = useI18n();
  const { pauseTask, resumeTask, cancelTask, retryFailed } = useBatchStore();

  const isUrl = task.sourcePath.startsWith("http://") || task.sourcePath.startsWith("https://");
  const fileName = isUrl
    ? task.sourcePath
    : task.sourcePath.split("/").pop()?.split("\\").pop() || task.sourcePath;
  const statusLabel = t(statusKey[task.status] || "taskStatus.pending");
  const progressWidth = task.status === "Completed" ? 100 : task.status === "Failed" ? 0 : Math.min(Math.round(task.progress * 100), 100);

  const progressIndicatorColor =
    task.status === "Completed" ? "bg-success"
    : task.status === "Failed" ? "bg-destructive"
    : "bg-primary";

  const elapsed = task.elapsedSecs > 0 ? formatDuration(task.elapsedSecs) : null;

  return (
    <div
      className={cn(
        "flex items-center gap-3 rounded-lg border p-3 transition-colors bg-card",
        task.status === "Failed" && "border-destructive/30 bg-destructive/5"
      )}
    >
      {isUrl ? (
        <Link size={16} className="text-muted-foreground shrink-0" />
      ) : (
        <FileText size={16} className="text-muted-foreground shrink-0" />
      )}

      <div className="flex-1 min-w-0">
        <div className="flex items-center gap-2">
          <Tooltip>
            <TooltipTrigger asChild>
              <span className="font-medium text-sm truncate cursor-help max-w-[200px]">{fileName}</span>
            </TooltipTrigger>
            <TooltipContent side="bottom" align="start" className="max-w-xs text-xs">
              {fileName}
            </TooltipContent>
          </Tooltip>
          <Badge variant={statusBadgeVariant[task.status] ?? "secondary"} className="text-[10px] px-1.5 py-0">
            {statusLabel}
          </Badge>
        </div>

        <div className="mt-1.5 flex items-center gap-2">
          <Progress value={progressWidth} indicatorClassName={progressIndicatorColor} className="h-1.5" />
          <span className="text-xs text-muted-foreground w-8 text-right tabular-nums">{progressWidth}%</span>
          {elapsed && (
            <span className="text-[10px] text-muted-foreground tabular-nums">{elapsed}</span>
          )}
        </div>

        {task.error && (
          <Tooltip>
            <TooltipTrigger asChild>
              <p className="text-xs text-destructive mt-1 line-clamp-2 break-words cursor-help">{task.error}</p>
            </TooltipTrigger>
            <TooltipContent side="bottom" align="start" className="max-w-xs p-2 text-xs">
              <p className="whitespace-pre-wrap break-words">{task.error}</p>
            </TooltipContent>
          </Tooltip>
        )}
      </div>

      <div className="flex items-center gap-0.5 shrink-0">
        {task.status === "Processing" && (
          <Tooltip>
            <TooltipTrigger asChild>
              <Button variant="ghost" size="icon" className="h-7 w-7" onClick={() => pauseTask(task.id)}>
                <Pause size={14} />
              </Button>
            </TooltipTrigger>
            <TooltipContent>{t("batch.pause")}</TooltipContent>
          </Tooltip>
        )}
        {task.status === "Paused" && (
          <Tooltip>
            <TooltipTrigger asChild>
              <Button variant="ghost" size="icon" className="h-7 w-7" onClick={() => resumeTask(task.id)}>
                <Play size={14} />
              </Button>
            </TooltipTrigger>
            <TooltipContent>{t("batch.resume")}</TooltipContent>
          </Tooltip>
        )}
        {task.status === "Failed" && (
          <Tooltip>
            <TooltipTrigger asChild>
              <Button variant="ghost" size="icon" className="h-7 w-7" onClick={() => retryFailed()}>
                <RotateCcw size={14} />
              </Button>
            </TooltipTrigger>
            <TooltipContent>{t("batch.retryFailed")}</TooltipContent>
          </Tooltip>
        )}
        {(task.status === "Pending" || task.status === "Paused") && (
          <Tooltip>
            <TooltipTrigger asChild>
              <Button variant="ghost" size="icon" className="h-7 w-7" onClick={() => cancelTask(task.id)}>
                <X size={14} />
              </Button>
            </TooltipTrigger>
            <TooltipContent>{t("taskStatus.cancelled")}</TooltipContent>
          </Tooltip>
        )}
      </div>
    </div>
  );
}

function formatDuration(secs: number): string {
  if (secs < 60) return `${secs}s`;
  const m = Math.floor(secs / 60);
  const s = secs % 60;
  return `${m}m ${s}s`;
}