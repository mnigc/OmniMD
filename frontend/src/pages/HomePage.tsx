import { useCallback, useEffect, useState } from "react";
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
import { pickOutputDir } from "../api/dialogs";
import { confirm } from "@tauri-apps/plugin-dialog";
import { useBatchStore } from "../store/useBatchStore";
import type { ConversionTask } from "../types";
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
import { ScrollArea } from "../components/ui/scroll-area";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "../components/ui/tooltip";

export function HomePage() {
  const { t } = useI18n();
  const { tasks, start, loading, cancelAll, retryFailed, clearDone, enqueue, concurrency, setConcurrency } = useBatchStore();
  const { defaultOutputDir } = useSettingsStore();

  const [outputDir, setOutputDir] = useState(defaultOutputDir);
  const [outputLocationMode, setOutputLocationMode] = useState<"sourceDir" | "custom">("sourceDir");
  const [supportedFormats, setSupportedFormats] = useState<string[]>([]);

  useEffect(() => {
    getSupportedFormats().then(setSupportedFormats).catch(() => {});
    (async () => {
      try {
        const state = useSettingsStore.getState();
        if (state.defaultOutputDir) return;
        const dir = await getDefaultOutputDir();
        if (dir) state.setDefaultOutputDir(dir);
      } catch {
        // ignore
      }
    })();
  }, []);

  useEffect(() => {
    if (outputLocationMode === "custom" && !outputDir)
      setOutputDir(defaultOutputDir);
  }, [defaultOutputDir, outputLocationMode]);

  useEffect(() => {
    useBatchStore.getState().refreshTasks();
    useBatchStore.getState().refreshSummary();
  }, []);

  const inferOutputDir = useCallback(
    (path: string): string => {
      if (outputLocationMode === "custom") return outputDir || ".";
      return path.replace(/\\/g, "/").split("/").slice(0, -1).join("/") || ".";
    },
    [outputDir, outputLocationMode, defaultOutputDir]
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
          showToast(t("toast.filePickFailed"), 3000);
        }
      }
      await useBatchStore.getState().refreshTasks();
      await useBatchStore.getState().refreshSummary();
    },
    [inferOutputDir, enqueue, t]
  );

  const addInputPaths = useCallback(async (paths: string[]) => {
    if (!paths.length) return;
    let expanded: string[] = [];
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
  }, [handleFiles]);

  const handleFolder = useCallback(
    (folderPath: string) => {
      if (!folderPath) return;
      addInputPaths([folderPath]);
    },
    [addInputPaths]
  );

  const handleBrowseOutputDir = useCallback(async () => {
    const dir = await pickOutputDir();
    if (dir) setOutputDir(dir);
  }, []);

  const handleOpenOutputDir = useCallback(async () => {
    try {
      await openFolder(outputDir);
    } catch {
      // ignore
    }
  }, [outputDir]);

  const orderedTasks = [...tasks]
    .sort((a, b) => {
      const rank = (s: string) => (s === "Processing" ? 0 : s === "Pending" ? 1 : s === "Failed" ? 3 : 2);
      return rank(a.status) - rank(b.status);
    });
  const MAX_RENDERED_TASKS = 200;
  const visibleTasks = orderedTasks.slice(0, MAX_RENDERED_TASKS);
  const hiddenCount = orderedTasks.length - visibleTasks.length;
  const totalTasks = tasks.length;
  const pendingCount = tasks.filter((t) => t.status === "Pending").length;
  const processingCount = tasks.filter((t) => t.status === "Processing").length;
  const completedCount = tasks.filter((t) => t.status === "Completed").length;
  const failedCount = tasks.filter((t) => t.status === "Failed").length;

  const StatusChip = ({ icon: Icon, count, label, color }: {
    icon: typeof Loader2;
    count: number;
    label: string;
    color: string;
  }) => (
    <div className="inline-flex items-center gap-1.5 rounded-full px-2.5 py-1 bg-muted/50 border border-border/60">
      <Icon size={12} className={color} />
      <span className="text-xs font-medium tabular-nums">{count}</span>
      <span className="text-xs text-muted-foreground">{label}</span>
    </div>
  );

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
                  value={outputDir}
                  onChange={(e) => setOutputDir(e.target.value)}
                  placeholder={t("home.outputDirPlaceholder")}
                  className="flex-1 min-w-0 h-7 text-xs"
                />
                <Button variant="outline" size="sm" onClick={handleBrowseOutputDir} className="h-7">
                  <FolderOpen size={13} />
                  {t("home.browse")}
                </Button>
                <Button variant="outline" size="sm" onClick={handleOpenOutputDir} disabled={!outputDir} title={t("home.openHint")} className="h-7">
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
                {failedCount > 0 && !tasks.some((t) => t.status === "Processing") && (
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
                  disabled={totalTasks === 0 || tasks.some((t) => t.status === "Processing")}
                  title={t("home.clearSession")}
                  className="h-7 w-7"
                >
                  <Trash2 size={13} />
                </Button>
              </div>
            </div>
            <div className="flex items-center gap-2 flex-wrap">
              <StatusChip icon={Loader2} count={processingCount} label={t("taskStatus.processing")} color="text-primary animate-spin" />
              <StatusChip icon={CheckCircle2} count={completedCount} label={t("taskStatus.completed")} color="text-success" />
              <StatusChip icon={XCircle} count={failedCount} label={t("taskStatus.failed")} color="text-destructive" />
              <StatusChip icon={AlertTriangle} count={pendingCount} label={t("taskStatus.pending")} color="text-warning" />
            </div>
          </CardHeader>

          <CardContent className="p-3 pt-0 shrink-0">
            {tasks.some((t) => t.status === "Processing") ? (
              <Button
                variant="destructive"
                className="w-full"
                onClick={async () => {
                  if (await confirm(t("home.cancelConfirm"), { title: t("home.cancel"), kind: "warning" })) {
                    cancelAll();
                  }
                }}
                disabled={loading}
              >
                <X size={14} />
                {t("home.cancel")}
              </Button>
            ) : (
              <Button className="w-full" onClick={start} disabled={pendingCount === 0}>
                <Play size={14} />
                {t("home.startConversion")}
              </Button>
            )}
          </CardContent>

          <CardContent className="flex-1 min-h-0 p-0">
            {totalTasks === 0 ? (
              <div className="flex flex-col items-center justify-center py-10 text-center h-full">
                <Inbox className="mx-auto mb-3 text-muted-foreground/40" size={32} />
                <p className="text-sm font-medium">{t("home.noFilesInSession")}</p>
                <p className="text-xs text-muted-foreground mt-1">
                  {t("dropzone.dropFilesOrFolder")}
                </p>
              </div>
            ) : (
              <ScrollArea className="h-full overflow-y-auto">
                <div className="space-y-1.5 p-3 pt-1">
                  {visibleTasks.map((tsk) => (
                    <TaskItem key={tsk.id} task={tsk as any} compact />
                  ))}
                  {hiddenCount > 0 && (
                    <p className="text-center text-xs text-muted-foreground py-2">
                      {t("batch.moreHidden").replace("{n}", String(hiddenCount))}
                    </p>
                  )}
                </div>
              </ScrollArea>
            )}
          </CardContent>
        </Card>
      </div>
    </div>
  );
}
