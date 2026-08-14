import {
  Pause,
  Play,
  X,
  Settings2,
  Loader2,
  CheckCircle2,
  XCircle,
  AlertTriangle,
  Inbox,
  Trash2,
} from "lucide-react";
import { TaskItem } from "../components/TaskItem";
import { PageHeader } from "../components/PageHeader";
import { useTaskStore } from "../store/useTaskStore";
import { useI18n } from "../i18n";
import { Button } from "../components/ui/button";
import { Input } from "../components/ui/input";

export function BatchPage() {
  const { t } = useI18n();
  const { tasks, cancelTask, clearCompleted, concurrency, setConcurrency } =
    useTaskStore();

  const processingTasks = tasks.filter((tsk) => tsk.status === "Processing");
  const completedTasks = tasks.filter((tsk) => tsk.status === "Completed");
  const failedTasks = tasks.filter((tsk) => tsk.status === "Failed");
  const pendingTasks = tasks.filter((tsk) => tsk.status === "Pending");

  return (
    <div className="h-full flex flex-col">
      <div className="flex-1 min-h-0 overflow-auto p-6">
        <div className="max-w-4xl mx-auto flex flex-col gap-6">
          <PageHeader
            title={t("batch.title")}
            actions={
              <div className="flex items-center gap-2">
                <Button variant="outline" size="sm">
                  <Pause size={14} />
                  {t("batch.pause")}
                </Button>
                <Button variant="outline" size="sm">
                  <Play size={14} />
                  {t("batch.resume")}
                </Button>
                <Button variant="outline" size="sm" onClick={clearCompleted}>
                  <Trash2 size={14} />
                  {t("batch.clearDone")}
                </Button>
                <Button
                  variant="destructive"
                  size="sm"
                  onClick={() =>
                    tasks.forEach(
                      (tsk) => tsk.status === "Processing" && cancelTask(tsk.id)
                    )
                  }
                >
                  <X size={14} />
                  {t("batch.cancelAll")}
                </Button>
                <div className="flex items-center gap-2 ml-3 pl-3 border-l border-border">
                  <Settings2 size={14} className="text-muted-foreground" />
                  <Input
                    type="number"
                    min={1}
                    max={16}
                    value={concurrency}
                    onChange={(e) =>
                      setConcurrency(parseInt(e.target.value) || 1)
                    }
                    className="w-16"
                    title={t("batch.concurrency")}
                  />
                </div>
              </div>
            }
          />

          <div className="space-y-2">
            {tasks.map((task) => (
              <TaskItem key={task.id} task={task} />
            ))}

            {tasks.length === 0 && (
              <div className="text-center py-16">
                <Inbox
                  className="mx-auto mb-4 text-muted-foreground opacity-30"
                  size={48}
                />
                <p className="text-lg font-medium">{t("batch.noTasks")}</p>
                <p className="text-sm text-muted-foreground mt-1">
                  {t("batch.noTasksHint")}
                </p>
              </div>
            )}
          </div>
        </div>
      </div>

      <div className="border-t border-border bg-muted/30 px-6 py-3 flex items-center gap-3 shrink-0">
        <div className="flex items-center gap-2 rounded-md bg-background px-2.5 py-1 border border-border">
          <Loader2 size={14} className="animate-spin text-primary" />
          <span className="text-sm font-medium tabular-nums">
            {processingTasks.length}
          </span>
          <span className="text-xs text-muted-foreground">
            {t("batch.processing")}
          </span>
        </div>
        <div className="flex items-center gap-2 rounded-md bg-background px-2.5 py-1 border border-border">
          <CheckCircle2 size={14} className="text-success" />
          <span className="text-sm font-medium tabular-nums">
            {completedTasks.length}
          </span>
          <span className="text-xs text-muted-foreground">
            {t("batch.completed")}
          </span>
        </div>
        <div className="flex items-center gap-2 rounded-md bg-background px-2.5 py-1 border border-border">
          <XCircle size={14} className="text-destructive" />
          <span className="text-sm font-medium tabular-nums">
            {failedTasks.length}
          </span>
          <span className="text-xs text-muted-foreground">
            {t("batch.failed")}
          </span>
        </div>
        <div className="flex items-center gap-2 rounded-md bg-background px-2.5 py-1 border border-border">
          <AlertTriangle size={14} className="text-warning" />
          <span className="text-sm font-medium tabular-nums">
            {pendingTasks.length}
          </span>
          <span className="text-xs text-muted-foreground">
            {t("batch.pending")}
          </span>
        </div>
        <div className="ml-auto text-xs text-muted-foreground">
          {t("batch.total")}:{" "}
          <span className="tabular-nums">{tasks.length}</span>
        </div>
      </div>
    </div>
  );
}