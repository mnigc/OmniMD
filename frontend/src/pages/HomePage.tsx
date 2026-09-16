import { useCallback, useEffect, useMemo, useState } from "react";
import {
  Folder, FolderOpen,
  Inbox,
  Loader2,
  Play,
  RotateCcw,
  Trash2,
  X,
  CheckCircle2,
  XCircle,
  AlertTriangle,
} from "lucide-react";
import { DropZone } from "../components/DropZone";
import { TaskItem } from "../components/TaskItem";
import { SellingPoints } from "../components/SellingPoints";
import {
  getDefaultOutputDir,
  openFolder,
  getSupportedFormats,
  listFilesInFolder,
} from "../api/tauriApi";
import { pickOutputDir, confirmDialog } from "../api/dialogs";
import { useBatchStore } from "../store/useBatchStore";
import { useSettingsStore } from "../store/useSettingsStore";
import { showToast } from "../lib/toast";
import { useI18n } from "../i18n";
import { cn } from "../lib/utils";
import { Button } from "../components/ui/button";
import { Input } from "../components/ui/input";
import { Label } from "../components/ui/label";
import {
  Card,
  CardContent,
  CardHeader,
  CardTitle,
} from "../components/ui/card";
import { VirtualList } from "../components/VirtualList";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "../components/ui/tooltip";

const MAX_RENDERED_TASKS = 200;

function StatusChip({
  icon: Icon,
  count,
  label,
  color,
  spin,
}: {
  icon: typeof Loader2;
  count: number;
  label: string;
  color: string;
  spin?: boolean;
}) {
  return (
    <div className="inline-flex items-center gap-1.5 rounded-full px-2.5 py-1 bg-muted/50 border border-border/60">
      <Icon size={12} className={cn(color, spin && count > 0 && "animate-spin")} />
      <span className="text-xs font-medium tabular-nums">{count}</span>
      <span className="text-xs text-muted-foreground">{label}</span>
    </div>
  );
}

export function HomePage() {
  const { t } = useI18n();
  const {
    tasks,
    start,
    loading,
    cancelAll,
    retryFailed,
    clearDone,
    enqueue,
    concurrency,
    setConcurrency,
  } = useBatchStore();
  const { defaultOutputDir, setDefaultOutputDir } = useSettingsStore();

  const [outputLocationMode, setOutputLocationMode] = useState<"sourceDir" | "custom">("sourceDir");
  const [supportedFormats, setSupportedFormats] = useState<string[]>([]);

  useEffect(() => {
    let alive = true;
    getSupportedFormats()
      .then((f) => {
        if (alive) setSupportedFormats(f);
      })
      .catch(() => {});
    (async () => {
      try {
        const state = useSettingsStore.getState();
        if (state.defaultOutputDir) return;
        const dir = await getDefaultOutputDir();
        if (alive && dir) state.setDefaultOutputDir(dir);
      } catch {
        // ignore
      }
    })();
    return () => {
      alive = false;
    };
  }, []);

  useEffect(() => {
    useBatchStore.getState().refreshTasks();
    useBatchStore.getState().refreshSummary();
  }, []);

  const inferOutputDir = useCallback(
    (path: string): string => {
      if (outputLocationMode === "custom") return defaultOutputDir || ".";
      return path.replace(/\\/g, "/").split("/").slice(0, -1).join("/") || ".";
    },
    [defaultOutputDir, outputLocationMode]
  );

  const handleFiles = useCallback(
    async (paths: string[]) => {
      if (paths.length === 0) return;
      const active = new Set(
        useBatchStore
          .getState()
          .tasks.filter(
            (t) => t.status !== "Completed" && t.status !== "Failed" && t.status !== "Cancelled"
          )
          .map((t) => t.sourcePath)
      );
      for (const path of paths) {
        if (active.has(path)) continue;
        active.add(path);
        const dir = inferOutputDir(path);
        const fileName = path.split(/[\\/]/).pop() || "output";
        const outputName = fileName.replace(/\.[^.]+$/, ".md");
        const outputPath = `${dir}/${outputName}`;
        const taskId = await enqueue(path, outputPath);
        if (!taskId) {
          console.error("Failed to enqueue:", path);
        }
      }
      await useBatchStore.getState().refreshTasks();
      await useBatchStore.getState().refreshSummary();
    },
    [inferOutputDir, enqueue]
  );

  const addInputPaths = useCallback(
    async (paths: string[]) => {
      if (!paths.length) return;
      const expanded: string[] = [];
      for (const p of paths) {
        try {
          expanded.push(...(await listFilesInFolder(p)));
        } catch {
          expanded.push(p);
        }
      }
      const unique = [...new Set(expanded)];
      if (unique.length === 0) return;
      handleFiles(unique);
    },
    [handleFiles]
  );

  const handleFolder = useCallback(
    (folderPath: string) => {
      if (!folderPath) return;
      addInputPaths([folderPath]);
    },
    [addInputPaths]
  );

  const handleBrowseOutputDir = useCallback(async () => {
    try {
      const dir = await pickOutputDir(t("home.outputDir"));
      if (dir) setDefaultOutputDir(dir);
    } catch (err) {
      showToast(
        err instanceof Error ? err.message : t("toast.folderPickFailed"),
        3000,
        "error"
      );
    }
  }, [setDefaultOutputDir, t]);

  const handleOpenOutputDir = useCallback(async () => {
    try {
      await openFolder(defaultOutputDir);
    } catch (err) {
      showToast(err instanceof Error ? err.message : t("history.openFolderError"), 3000, "error");
    }
  }, [defaultOutputDir, t]);

  const { visibleTasks, hiddenCount, totalTasks, pendingCount, processingCount, completedCount, failedCount, hasProcessing } =
    useMemo(() => {
      const rank = (s: string) =>
        s === "Processing" ? 0 : s === "Pending" ? 1 : s === "Failed" ? 3 : 2;
      const ordered = [...tasks].sort((a, b) => rank(a.status) - rank(b.status));
      const counts = { pending: 0, processing: 0, completed: 0, failed: 0 };
      for (const task of tasks) {
        if (task.status === "Pending") counts.pending++;
        else if (task.status === "Processing") counts.processing++;
        else if (task.status === "Completed") counts.completed++;
        else if (task.status === "Failed") counts.failed++;
      }
      return {
        visibleTasks: ordered.slice(0, MAX_RENDERED_TASKS),
        hiddenCount: ordered.length - Math.min(ordered.length, MAX_RENDERED_TASKS),
        totalTasks: tasks.length,
        pendingCount: counts.pending,
        processingCount: counts.processing,
        completedCount: counts.completed,
        failedCount: counts.failed,
        hasProcessing: counts.processing > 0,
      };
    }, [tasks]);

  return (
    <div className="h-full flex flex-col overflow-hidden">
      <div className="flex-1 min-h-0 w-full max-w-5xl mx-auto flex flex-col gap-3 p-4">
        {/* Hero — title / selling points / supported formats, stacked */}
        <div className="shrink-0">
          <h1 className="text-lg font-semibold tracking-tight">
            {t("home.title")}
          </h1>
          <div className="mt-1">
            <SellingPoints />
          </div>
          <p className="mt-1 text-xs text-muted-foreground/80 break-words">
            {t("home.supportedFormats")}
            {supportedFormats.map((f) => f.toUpperCase()).join(" / ")}
          </p>
        </div>

        {/* Card 1: 添加文件 — horizontal, compact */}
        <Card className="shrink-0">
          <CardContent className="p-3">
            <DropZone onFiles={addInputPaths} onFolder={handleFolder} formats={supportedFormats} />
          </CardContent>
        </Card>

        {/* Card 2: 转换设置 — single row */}
        <Card className="shrink-0">
          <CardContent className="p-3 flex flex-wrap items-center gap-x-5 gap-y-2">
            <div className="flex items-center gap-2">
              <Label className="text-xs text-muted-foreground whitespace-nowrap">
                {t("batch.concurrency")}
              </Label>
              <select
                value={concurrency}
                onChange={(e) => setConcurrency(Number(e.target.value))}
                className="h-7 rounded border border-border bg-background px-2 text-xs"
              >
                {[1, 2, 3, 4, 5].map((n) => (
                  <option key={n} value={n}>{n}</option>
                ))}
              </select>
            </div>

            <div className="flex items-center gap-2">
              <Label className="text-xs text-muted-foreground whitespace-nowrap">
                {t("home.outputLocation")}
              </Label>
              <div className="inline-flex rounded-lg bg-muted p-0.5">
                <button
                  type="button"
                  onClick={() => setOutputLocationMode("sourceDir")}
                  className={cn(
                    "flex items-center gap-1.5 h-7 px-2.5 rounded-md text-xs font-medium transition-colors",
                    outputLocationMode === "sourceDir"
                      ? "bg-background text-foreground shadow-sm"
                      : "text-muted-foreground hover:text-foreground"
                  )}
                >
                  <Inbox size={13} />
                  {t("home.outputInSourceDir")}
                </button>
                <button
                  type="button"
                  onClick={() => setOutputLocationMode("custom")}
                  className={cn(
                    "flex items-center gap-1.5 h-7 px-2.5 rounded-md text-xs font-medium transition-colors",
                    outputLocationMode === "custom"
                      ? "bg-background text-foreground shadow-sm"
                      : "text-muted-foreground hover:text-foreground"
                  )}
                >
                  <FolderOpen size={13} />
                  {t("home.outputCustom")}
                </button>
              </div>
            </div>

            {outputLocationMode === "custom" && (
              <div className="flex items-center gap-2 flex-1 min-w-[240px]">
                <Input
                  type="text"
                  value={defaultOutputDir}
                  onChange={(e) => setDefaultOutputDir(e.target.value)}
                  placeholder={t("home.outputDirPlaceholder")}
                  className="flex-1 min-w-0 h-7 text-xs"
                />
                <Button variant="outline" size="sm" onClick={handleBrowseOutputDir} className="h-7">
                  <FolderOpen size={13} />
                  {t("home.browse")}
                </Button>
                <Button variant="outline" size="sm" onClick={handleOpenOutputDir} disabled={!defaultOutputDir} title={t("home.openHint")} className="h-7">
                  <Folder size={13} />
                  {t("home.open")}
                </Button>
              </div>
            )}
          </CardContent>
        </Card>

        {/* Card 3: 待转换文件 — fills remaining height */}
        <Card className="flex flex-col overflow-hidden flex-1 min-h-0">
          <CardHeader className="p-3 pb-2 space-y-2 shrink-0">
            <div className="flex items-center justify-between gap-2">
              <CardTitle className="text-sm">
                {t("home.sessionTitle")}{" "}
                <span className="text-muted-foreground tabular-nums">({totalTasks})</span>
              </CardTitle>
              <div className="flex items-center gap-1">
                {failedCount > 0 && !hasProcessing && (
                  <Tooltip>
                    <TooltipTrigger asChild>
                      <Button variant="ghost" size="icon" onClick={retryFailed} className="h-7 w-7">
                        <RotateCcw size={13} />
                      </Button>
                    </TooltipTrigger>
                    <TooltipContent>{t("home.retryFailed")}</TooltipContent>
                  </Tooltip>
                )}
                <Button
                  variant="ghost"
                  size="icon"
                  onClick={clearDone}
                  disabled={totalTasks === 0 || hasProcessing}
                  title={t("home.clearSession")}
                  className="h-7 w-7"
                >
                  <Trash2 size={13} />
                </Button>
              </div>
            </div>
            <div className="flex items-center gap-2 flex-wrap">
              <StatusChip icon={Loader2} count={processingCount} label={t("taskStatus.processing")} color="text-primary" spin />
              <StatusChip icon={CheckCircle2} count={completedCount} label={t("taskStatus.completed")} color="text-success" />
              <StatusChip icon={XCircle} count={failedCount} label={t("taskStatus.failed")} color="text-destructive" />
              <StatusChip icon={AlertTriangle} count={pendingCount} label={t("taskStatus.pending")} color="text-warning" />
            </div>
          </CardHeader>

          <CardContent className="p-3 pt-0 shrink-0">
            {hasProcessing ? (
              <Button
                variant="destructive"
                className="w-full"
                onClick={async () => {
                  if (await confirmDialog(t("home.cancelConfirm"), t("home.cancel"))) {
                    cancelAll();
                  }
                }}
                disabled={loading}
              >
                <X size={14} />
                {t("home.cancel")}
              </Button>
            ) : (
              <Button className="w-full" onClick={start} disabled={pendingCount === 0 || loading}>
                <Play size={14} />
                {t("home.startConversion")}
              </Button>
            )}
          </CardContent>

          <CardContent className="flex-1 min-h-0 p-0 flex flex-col">
            {totalTasks === 0 ? (
              <div className="flex flex-col items-center justify-center py-10 text-center h-full">
                <Inbox className="mx-auto mb-3 text-muted-foreground/40" size={32} />
                <p className="text-sm font-medium">{t("home.noFilesInSession")}</p>
                <p className="text-xs text-muted-foreground mt-1">
                  {t("dropzone.dropFilesOrFolder")}
                </p>
              </div>
            ) : (
              <>
                <VirtualList
                  items={visibleTasks}
                  className="flex-1 min-h-0 px-3 pt-1"
                  estimateSize={56}
                  gap={6}
                  itemKey={(tsk) => tsk.id}
                  renderItem={(tsk) => <TaskItem task={tsk} compact />}
                />
                {hiddenCount > 0 && (
                  <p className="text-center text-xs text-muted-foreground py-2 shrink-0">
                    {t("batch.moreHidden", { n: MAX_RENDERED_TASKS })}
                  </p>
                )}
              </>
            )}
          </CardContent>
        </Card>
      </div>
    </div>
  );
}
