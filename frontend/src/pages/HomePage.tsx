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
import { useModelStore } from "../store/useModelStore";
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

const CLOUD_EXTENSIONS = [
  "pdf", "docx", "pptx", "xlsx",
  "png", "jpg", "jpeg", "jp2", "webp", "gif", "bmp",
];

export function HomePage() {
  const { t } = useI18n();
  const { tasks, summary, start, loading, cancelAll, retryFailed, clearDone, enqueue, setPanelOpen } = useBatchStore();
  const { outputMode, defaultOutputDir, allowOnline } = useSettingsStore();
  const { engineMode } = useModelStore();

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
      let files = paths;

      if (engineMode === "cloud") {
        const skipped: string[] = [];
        const kept: string[] = [];
        for (const path of paths) {
          const slash = Math.max(path.lastIndexOf("/"), path.lastIndexOf("\\"));
          const dot = path.lastIndexOf(".");
          const ext = dot > slash ? path.slice(dot + 1).toLowerCase() : "";
          if (ext && !CLOUD_EXTENSIONS.includes(ext)) {
            skipped.push(path);
          } else {
            kept.push(path);
          }
        }
        files = kept;
        if (skipped.length > 0) {
          const names = skipped
            .map((s) => s.split(/[\\/]/).pop() || s)
            .join(", ");
          showToast(
            t("dropzone.cloudSkipToast")
              .replace("{n}", String(skipped.length))
              .replace("{files}", names),
            4000
          );
        }
      }

      if (files.length === 0) return;
      for (const path of files) {
        const dir = inferOutputDir(path);
        const fileName = path.split(/[\\/]/).pop() || "output";
        const outputName = fileName.replace(/\.[^.]+$/, ".md");
        const outputPath = `${dir}/${outputName}`;
        await enqueue(path, outputPath, outputMode);
      }
    },
    [inferOutputDir, outputMode, enqueue, engineMode, t]
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
      await enqueue(url, outputPath, outputMode);
      setUrlInput("");
    } catch (err: any) {
      setUrlError(err.message || String(err));
    } finally {
      setDownloading(false);
    }
  }, [urlInput, outputMode, inferOutputDir, allowOnline, enqueue, t]);

  const previewTasks = tasks.filter((t) => t.status === "Pending" || t.status === "Processing").slice(0, 5);
  const totalTasks = tasks.length;
  const pendingCount = tasks.filter((t) => t.status === "Pending").length;
  const processingCount = tasks.filter((t) => t.status === "Processing").length;
  const completedCount = tasks.filter((t) => t.status === "Completed").length;
  const failedCount = tasks.filter((t) => t.status === "Failed").length;
  const hasTerminal = completedCount + failedCount > 0;

  return (
    <div className="h-full flex flex-col">
      <div className="flex-1 p-6 overflow-auto">
        <div className="max-w-5xl mx-auto w-full flex flex-col gap-6">
          <div className="text-center mb-2">
            <h1 className="text-xl font-semibold tracking-tight">{t("home.title")}</h1>
            <p className="text-muted-foreground mt-1.5 text-sm">{t("home.subtitle")}</p>
            <SellingPoints className="mt-4" />
          </div>

          <DropZone onFiles={addInputPaths} onFolder={handleFolder} formats={supportedFormats} />
          <OutputModeSelector />

          <div className="flex flex-col gap-2">
            <Label className="text-xs text-muted-foreground">{t("home.outputLocation")}</Label>
            <div className="flex items-center gap-2">
              <button
                type="button"
                onClick={() => setOutputLocationMode("sourceDir")}
                className={cn(
                  "flex items-center gap-1.5 h-9 px-3 rounded-md border text-xs font-medium whitespace-nowrap transition-all",
                  outputLocationMode === "sourceDir"
                    ? "border-primary bg-primary/5 text-primary"
                    : "border-border bg-muted/30 text-muted-foreground hover:border-primary/40 hover:bg-muted/50"
                )}
              >
                <Inbox size={14} />
                {t("home.outputInSourceDir")}
              </button>
              <button
                type="button"
                onClick={() => setOutputLocationMode("custom")}
                className={cn(
                  "flex items-center gap-1.5 h-9 px-3 rounded-md border text-xs font-medium whitespace-nowrap transition-all",
                  outputLocationMode === "custom"
                    ? "border-primary bg-primary/5 text-primary"
                    : "border-border bg-muted/30 text-muted-foreground hover:border-primary/40 hover:bg-muted/50"
                )}
              >
                <FolderOpen size={14} />
                {t("home.outputCustom")}
              </button>
              {outputLocationMode === "custom" && (
                <>
                  <Input
                    type="text"
                    value={outputDir}
                    onChange={(e) => setOutputDir(e.target.value)}
                    placeholder={t("home.outputDirPlaceholder")}
                    className="flex-1 min-w-0 h-9"
                  />
                  <Button variant="outline" onClick={handleBrowseOutputDir}>
                    <FolderOpen size={14} />
                    {t("home.browse")}
                  </Button>
                  <Button variant="outline" onClick={handleOpenOutputDir} disabled={!outputDir} title={t("home.openHint")}>
                    <Folder size={14} />
                    {t("home.open")}
                  </Button>
                </>
              )}
            </div>
          </div>

          <div className="relative w-full max-w-52">
            <Link size={14} className="absolute left-2.5 top-1/2 -translate-y-1/2 text-muted-foreground pointer-events-none" />
            <Input
              type="text"
              placeholder={allowOnline ? t("home.pasteUrl") : t("home.pasteUrlDisabled")}
              value={urlInput}
              onChange={(e) => setUrlInput(e.target.value)}
              onKeyDown={(e) => { if (e.key === "Enter") handleUrlSubmit(); }}
              disabled={downloading || !allowOnline}
              className="pl-8 text-xs"
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
            <p className="text-xs text-muted-foreground -mt-2">{t("home.urlPrivacyNote")}</p>
          )}

          <Card className="flex flex-col overflow-hidden">
            <CardHeader className="pb-2">
              <div className="flex items-center justify-between">
                <CardTitle className="text-sm">
                  {t("home.sessionTitle")}{" "}
                  <span className="text-muted-foreground tabular-nums">({totalTasks})</span>
                </CardTitle>
              </div>
            </CardHeader>

            <CardContent className="flex-1 min-h-0 p-0">
              {totalTasks === 0 ? (
                <div className="flex flex-col items-center justify-center py-12 text-center">
                  <Inbox className="mx-auto mb-3 text-muted-foreground opacity-30" size={32} />
                  <p className="text-sm font-medium">{t("home.noFilesInSession")}</p>
                </div>
              ) : (
                <ScrollArea className="min-h-[80px] max-h-[240px] overflow-y-auto">
                  <div className="space-y-2 p-4 pt-1">
                    {previewTasks.map((tsk) => (
                      <TaskItem key={tsk.id} task={tsk as any} compact />
                    ))}
                    {totalTasks > 5 && (
                      <button
                        onClick={() => setPanelOpen(true)}
                        className="w-full text-center text-xs text-primary py-2 hover:underline"
                      >
                        {t("home.viewAll")} ({totalTasks - 5} more)
                      </button>
                    )}
                  </div>
                </ScrollArea>
              )}
            </CardContent>

            <CardFooter className="border-t border-border p-4 flex flex-col gap-3">
              <div className="flex items-center gap-2 w-full">
                {tasks.some((t) => t.status === "Processing") ? (
                  <Button variant="destructive" onClick={async () => {
                    if (await confirm('确定要取消所有进行中的转换吗？', { title: '取消转换', kind: 'warning' })) {
                      cancelAll();
                    }
                  }} disabled={loading}>
                    <X size={16} />
                    {t("home.cancel")}
                  </Button>
                ) : (
                  <Button onClick={start} disabled={pendingCount === 0}>
                    <Play size={16} />
                    {t("home.startConversion")}
                  </Button>
                )}
                {failedCount > 0 && !tasks.some((t) => t.status === "Processing") && (
                  <Button variant="outline" onClick={retryFailed} className="text-destructive hover:text-destructive">
                    <RotateCcw size={16} />
                    {t("batch.retryFailed")}
                  </Button>
                )}
                <Button variant="outline" onClick={clearDone} disabled={totalTasks === 0 || tasks.some((t) => t.status === "Processing")}>
                  <Trash2 size={16} />
                  {t("home.clearSession")}
                </Button>
              </div>

              <div className="flex items-center gap-2 flex-wrap">
                <div className="flex items-center gap-1.5 rounded-md bg-background px-2 py-1 border border-border">
                  <Loader2 size={12} className="animate-spin text-primary" />
                  <span className="text-xs font-medium tabular-nums">{processingCount}</span>
                  <span className="text-[11px] text-muted-foreground">{t("taskStatus.processing")}</span>
                </div>
                <div className="flex items-center gap-1.5 rounded-md bg-background px-2 py-1 border border-border">
                  <CheckCircle2 size={12} className="text-success" />
                  <span className="text-xs font-medium tabular-nums">{completedCount}</span>
                  <span className="text-[11px] text-muted-foreground">{t("taskStatus.completed")}</span>
                </div>
                <div className="flex items-center gap-1.5 rounded-md bg-background px-2 py-1 border border-border">
                  <XCircle size={12} className="text-destructive" />
                  <span className="text-xs font-medium tabular-nums">{failedCount}</span>
                  <span className="text-[11px] text-muted-foreground">{t("taskStatus.failed")}</span>
                </div>
                <div className="flex items-center gap-1.5 rounded-md bg-background px-2 py-1 border border-border">
                  <AlertTriangle size={12} className="text-warning" />
                  <span className="text-xs font-medium tabular-nums">{pendingCount}</span>
                  <span className="text-[11px] text-muted-foreground">{t("taskStatus.pending")}</span>
                </div>
              </div>
            </CardFooter>
          </Card>
        </div>
      </div>
    </div>
  );
}