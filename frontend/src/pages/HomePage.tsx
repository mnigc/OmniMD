import { useCallback, useEffect, useState } from "react";
import {
  Folder, FolderOpen,
  Inbox,
  Link,
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
import { OutputModeSelector } from "../components/OutputModeSelector";
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
  CardFooter,
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
  const { tasks, start, loading, cancelAll, retryFailed, clearDone, enqueue, setPanelOpen } = useBatchStore();
  const { outputMode, defaultOutputDir, allowOnline, parseQuality } = useSettingsStore();

  const [outputDir, setOutputDir] = useState(defaultOutputDir);
  const [outputLocationMode, setOutputLocationMode] = useState<"sourceDir" | "custom">("sourceDir");
  const [supportedFormats, setSupportedFormats] = useState<string[]>([]);
  const [urlInput, setUrlInput] = useState("");
  const [downloading, setDownloading] = useState(false);
  const [urlError, setUrlError] = useState("");

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
    [outputDir, outputLocationMode]
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
        const taskId = await enqueue(path, outputPath, outputMode, parseQuality);
        if (!taskId) {
          console.error("Failed to enqueue:", path);
          showToast(t("toast.filePickFailed"), 3000);
        }
      }
      await useBatchStore.getState().refreshTasks();
      await useBatchStore.getState().refreshSummary();
    },
[inferOutputDir, outputMode, parseQuality, enqueue, t]
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

  const handleUrlSubmit = useCallback(async () => {
    const url = urlInput.trim();
    if (!url) return;
    if (!allowOnline) {
      setUrlError(t("home.urlNotAllowed"));
      return;
    }
    setDownloading(true);
    setUrlError("");
    try {
      const dir = inferOutputDir(url);
      const urlName = url.split("/").pop()?.split("?")[0] || "page";
      const outputName = urlName.replace(/\.[^.]+$/, "") + ".md";
      const outputPath = `${dir}/${outputName}`;
      await enqueue(url, outputPath, outputMode, parseQuality);
      setUrlInput("");
    } catch (err: any) {
      setUrlError(err.message || String(err));
    } finally {
      setDownloading(false);
    }
  }, [urlInput, outputMode, parseQuality, inferOutputDir, allowOnline, enqueue, t]);

  const previewTasks = [...tasks]
    .sort((a, b) => {
      const rank = (s: string) => (s === "Processing" ? 0 : s === "Pending" ? 1 : s === "Failed" ? 3 : 2);
      return rank(a.status) - rank(b.status);
    })
    .slice(0, 8);
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
    <div className="inline-flex items-center gap-1.5 rounded-full px-2.5 py-1 bg-muted/40 border border-border/60">
      <Icon size={10} className={color} />
      <span className="text-xs font-medium tabular-nums">{count}</span>
      <span className="text-[10px] text-muted-foreground">{label}</span>
    </div>
  );

  return (
    <div className="h-full flex flex-col">
      <div className="flex-1 p-6 overflow-auto">
        <div className="max-w-7xl mx-auto w-full flex flex-col gap-5">
          <div className="text-center">
            <h1 className="text-xl font-semibold tracking-tight">{t("home.title")}</h1>
            <p className="text-muted-foreground mt-1 text-sm">{t("home.subtitle")}</p>
            <SellingPoints className="mt-3" />
          </div>

          <div className="grid grid-cols-12 gap-5 flex-1 min-h-0">
            <div className="col-span-7 flex flex-col gap-4 min-h-0">
              <DropZone onFiles={addInputPaths} onFolder={handleFolder} formats={supportedFormats} />

              <OutputModeSelector />

              <div className="flex flex-col gap-2">
                <Label className="text-xs text-muted-foreground">{t("home.outputLocation")}</Label>
                <div className="flex items-center gap-2 flex-wrap">
                  <button
                    type="button"
                    onClick={() => setOutputLocationMode("sourceDir")}
                    className={cn(
                      "flex items-center gap-1.5 h-8 px-3 rounded-md border text-xs font-medium whitespace-nowrap transition-all",
                      outputLocationMode === "sourceDir"
                        ? "border-primary bg-primary/5 text-primary shadow-sm"
                        : "border-border bg-muted/30 text-muted-foreground hover:border-primary/40 hover:bg-muted/50"
                    )}
                  >
                    <Inbox size={13} />
                    {t("home.outputInSourceDir")}
                  </button>
                  <button
                    type="button"
                    onClick={() => setOutputLocationMode("custom")}
                    className={cn(
                      "flex items-center gap-1.5 h-8 px-3 rounded-md border text-xs font-medium whitespace-nowrap transition-all",
                      outputLocationMode === "custom"
                        ? "border-primary bg-primary/5 text-primary shadow-sm"
                        : "border-border bg-muted/30 text-muted-foreground hover:border-primary/40 hover:bg-muted/50"
                    )}
                  >
                    <FolderOpen size={13} />
                    {t("home.outputCustom")}
                  </button>
                  {outputLocationMode === "custom" && (
                    <div className="flex items-center gap-2 flex-1 min-w-[200px]">
                      <Input
                        type="text"
                        value={outputDir}
                        onChange={(e) => setOutputDir(e.target.value)}
                        placeholder={t("home.outputDirPlaceholder")}
                        className="flex-1 min-w-0 h-8 text-xs"
                      />
                      <Button variant="outline" size="sm" onClick={handleBrowseOutputDir} className="h-8">
                        <FolderOpen size={13} />
                        {t("home.browse")}
                      </Button>
                      <Button variant="outline" size="sm" onClick={handleOpenOutputDir} disabled={!outputDir} title={t("home.openHint")} className="h-8">
                        <Folder size={13} />
                        {t("home.open")}
                      </Button>
                    </div>
                  )}
                </div>
              </div>

              <div className="relative w-full max-w-md">
                <Link size={13} className="absolute left-2.5 top-1/2 -translate-y-1/2 text-muted-foreground pointer-events-none" />
                <Input
                  type="text"
                  placeholder={allowOnline ? t("home.pasteUrl") : t("home.pasteUrlDisabled")}
                  value={urlInput}
                  onChange={(e) => setUrlInput(e.target.value)}
                  onKeyDown={(e) => { if (e.key === "Enter") handleUrlSubmit(); }}
                  disabled={downloading || !allowOnline}
                  className="pl-8 text-xs h-8"
                />
                {urlError && (
                  <Tooltip>
                    <TooltipTrigger asChild>
                      <span className="absolute -bottom-5 left-2.5 text-xs text-destructive truncate max-w-full cursor-help">{urlError}</span>
                    </TooltipTrigger>
                    <TooltipContent side="top" className="max-w-xs p-2 text-xs">
                      <p className="whitespace-pre-wrap break-words">{urlError}</p>
                    </TooltipContent>
                  </Tooltip>
                )}
              </div>
              {urlInput && (
                <p className="text-xs text-muted-foreground -mt-1">{t("home.urlPrivacyNote")}</p>
              )}

              <div className="mt-auto pt-2 flex items-center gap-2 flex-wrap">
                <StatusChip icon={Loader2} count={processingCount} label={t("taskStatus.processing")} color="text-primary animate-spin" />
                <StatusChip icon={CheckCircle2} count={completedCount} label={t("taskStatus.completed")} color="text-success" />
                <StatusChip icon={XCircle} count={failedCount} label={t("taskStatus.failed")} color="text-destructive" />
                <StatusChip icon={AlertTriangle} count={pendingCount} label={t("taskStatus.pending")} color="text-warning" />
              </div>
            </div>

            <div className="col-span-5 flex flex-col min-h-0">
              <Card className="flex flex-col overflow-hidden flex-1 min-h-0">
                <CardHeader className="pb-2">
                  <div className="flex items-center justify-between">
                    <CardTitle className="text-sm">
                      {t("home.sessionTitle")}{" "}
                      <span className="text-muted-foreground tabular-nums">({totalTasks})</span>
                    </CardTitle>
                    <div className="flex items-center gap-1">
                      {tasks.some((t) => t.status === "Processing") ? (
                        <Button variant="destructive" size="sm" onClick={async () => {
                          if (await confirm('确定要取消所有进行中的转换吗？', { title: '取消转换', kind: 'warning' })) {
                            cancelAll();
                          }
                        }} disabled={loading} className="h-7 text-xs">
                          <X size={12} />
                          {t("home.cancel")}
                        </Button>
                      ) : (
                        <Button size="sm" onClick={start} disabled={pendingCount === 0} className="h-7 text-xs">
                          <Play size={12} />
                          {t("home.startConversion")}
                        </Button>
                      )}
                      {failedCount > 0 && !tasks.some((t) => t.status === "Processing") && (
                        <Button variant="outline" size="sm" onClick={retryFailed} className="h-7 text-xs">
                          <RotateCcw size={12} />
                        </Button>
                      )}
                      <Button variant="outline" size="sm" onClick={clearDone} disabled={totalTasks === 0 || tasks.some((t) => t.status === "Processing")} className="h-7 text-xs">
                        <Trash2 size={12} />
                      </Button>
                    </div>
                  </div>
                </CardHeader>

                <CardContent className="flex-1 min-h-0 p-0">
                  {totalTasks === 0 ? (
                    <div className="flex flex-col items-center justify-center py-16 text-center h-full">
                      <Inbox className="mx-auto mb-3 text-muted-foreground/40" size={36} />
                      <p className="text-sm font-medium">{t("home.noFilesInSession")}</p>
                      <p className="text-xs text-muted-foreground mt-1">
                        {t("dropzone.dropFilesOrFolder")}
                      </p>
                    </div>
                  ) : (
                    <ScrollArea className="h-full max-h-[420px] overflow-y-auto">
                      <div className="space-y-1.5 p-3 pt-1">
                        {previewTasks.map((tsk) => (
                          <TaskItem key={tsk.id} task={tsk as any} compact />
                        ))}
                        {totalTasks > 8 && (
                          <button
                            onClick={() => setPanelOpen(true)}
                            className="w-full text-center text-xs text-primary py-2 hover:underline transition-colors"
                          >
                            {t("home.viewAll")} ({totalTasks - 8} more)
                          </button>
                        )}
                      </div>
                    </ScrollArea>
                  )}
                </CardContent>
              </Card>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}