import { useCallback, useEffect, useState } from "react";
import { FilePlus, FolderOpen, Link, Loader2, Trash2, RotateCcw } from "lucide-react";
import { DropZone } from "../components/DropZone";
import { TaskItem } from "../components/TaskItem";
import { convertBatch, convertFile, getSupportedFormats } from "../api/tauriApi";
import { pickFiles, pickOutputDir } from "../api/dialogs";
import { useTaskStore } from "../store/useTaskStore";
import { useI18n } from "../i18n";
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

type Page = "home" | "convert" | "batch" | "settings";

interface HomePageProps {
  onNavigate: (page: Page) => void;
}

export function HomePage({ onNavigate }: HomePageProps) {
  const { t } = useI18n();
  const {
    tasks,
    addTasks,
    finalizeTask,
    failTask,
    clearCompleted,
    setCurrentTask,
    concurrency,
    setConcurrency,
  } = useTaskStore();

  const [outputDir, setOutputDir] = useState("");
  const [converting, setConverting] = useState(false);
  const [supportedFormats, setSupportedFormats] = useState<string[]>([]);

  useEffect(() => {
    getSupportedFormats().then(setSupportedFormats).catch(() => {});
  }, []);

  const handleFiles = useCallback(
    async (paths: string[]) => {
      if (paths.length === 0) return;

      const dir =
        outputDir ||
        paths[0].replace(/\\/g, "/").split("/").slice(0, -1).join("/");
      const placeholders = addTasks(paths, dir);

      setConverting(true);

      if (paths.length === 1) {
        try {
          const result = await convertFile(paths[0], dir);
          finalizeTask(placeholders[0].id, result.taskId, result, null);
          setCurrentTask(
            {
              id: result.taskId,
              sourcePath: paths[0],
              outputPath: `${dir}/${paths[0].split(/[\\/]/).pop()}`,
              status: "Completed",
              progress: 1,
              stage: "Saving",
              error: null,
              createdAt: Date.now(),
              completedAt: Date.now(),
            },
            result
          );
          onNavigate("convert");
        } catch (err: any) {
          failTask(placeholders[0]?.id || "", err.message || String(err));
        } finally {
          setConverting(false);
        }
      } else {
        try {
          const batchResult = await convertBatch(paths, dir, concurrency);

          batchResult.results.forEach((r, index) => {
            const placeholder = placeholders[index];
            if (!placeholder) return;
            if (r.success) {
              finalizeTask(placeholder.id, r.taskId, r, null);
            } else {
              finalizeTask(
                placeholder.id,
                r.taskId,
                r,
                r.errors[0]?.message || "Unknown error"
              );
            }
          });

          if (batchResult.completed > 0) {
            onNavigate("batch");
          }
        } catch (err: any) {
          placeholders.forEach((p) =>
            failTask(p.id, err.message || String(err))
          );
        } finally {
          setConverting(false);
        }
      }
    },
    [
      addTasks,
      finalizeTask,
      failTask,
      setCurrentTask,
      onNavigate,
      outputDir,
      concurrency,
    ]
  );

  const handleBrowseOutputDir = useCallback(async () => {
    const dir = await pickOutputDir();
    if (dir) setOutputDir(dir);
  }, []);

  const handleAddFiles = useCallback(async () => {
    const paths = await pickFiles(supportedFormats);
    if (paths.length > 0) {
      handleFiles(paths);
    }
  }, [handleFiles, supportedFormats]);

  const totalTasks = tasks.length;
  const completedTasks = tasks.filter((tsk) => tsk.status === "Completed").length;
  const failedTasks = tasks.filter((tsk) => tsk.status === "Failed").length;

  const activeTasks = tasks.filter(
    (tsk) => tsk.status !== "Completed" && tsk.status !== "Cancelled"
  );

  return (
    <div className="h-full flex flex-col">
      <div className="flex-1 p-6 flex flex-col gap-6 overflow-auto">
        <div className="max-w-6xl mx-auto w-full flex flex-col gap-6 flex-1">
        <div className="text-center mb-2">
          <h1 className="text-xl font-semibold tracking-tight">
            {t("home.title")}
          </h1>
          <p className="text-muted-foreground mt-1.5 text-sm">
            {t("home.subtitle")}
          </p>
        </div>

        <div className="grid grid-cols-1 lg:grid-cols-3 gap-6 flex-1">
          <div className="lg:col-span-2 flex flex-col gap-4">
            <div className="flex flex-wrap gap-x-6 gap-y-4">
              <div className="flex-1 min-w-0 flex flex-col gap-1.5">
                <Label className="text-xs text-muted-foreground">
                  {t("home.outputDir")}
                </Label>
                <div className="flex gap-2">
                  <Input
                    type="text"
                    value={outputDir}
                    onChange={(e) => setOutputDir(e.target.value)}
                    placeholder={t("home.outputDirPlaceholder")}
                    className="flex-1 min-w-0"
                  />
                  <Button variant="outline" onClick={handleBrowseOutputDir}>
                    <FolderOpen size={14} />
                    {t("home.browse")}
                  </Button>
                </div>
              </div>
              <div className="w-24 flex flex-col gap-1.5 shrink-0">
                <Label className="text-xs text-muted-foreground">
                  {t("home.concurrency")}
                </Label>
                <Input
                  type="number"
                  min={1}
                  max={16}
                  value={concurrency}
                  onChange={(e) => setConcurrency(parseInt(e.target.value) || 1)}
                />
              </div>
            </div>

            <DropZone
              onFiles={handleFiles}
              disabled={converting}
              formats={supportedFormats}
              className="flex-1 min-h-[220px]"
            />

            <div className="flex items-center gap-3">
              <Button onClick={handleAddFiles} disabled={converting}>
                <FilePlus size={16} />
                {t("home.addFiles")}
              </Button>

              <div className="relative flex-1 max-w-52">
                <Link
                  size={14}
                  className="absolute left-2.5 top-1/2 -translate-y-1/2 text-muted-foreground pointer-events-none"
                />
                <Input
                  type="text"
                  placeholder={t("home.pasteUrl")}
                  disabled
                  className="pl-8 text-xs"
                />
              </div>
            </div>
          </div>

          <Card className="flex flex-col h-full overflow-hidden">
            <CardHeader className="pb-3">
              <div className="flex items-center justify-between">
                <CardTitle className="text-sm">
                  {t("home.recentConversions")}
                </CardTitle>
                <div className="flex items-center gap-0.5">
                  <Button
                    variant="ghost"
                    size="icon"
                    className="h-7 w-7"
                    onClick={() => onNavigate("batch")}
                    title={t("home.viewAll")}
                  >
                    <RotateCcw size={14} />
                  </Button>
                  <Button
                    variant="ghost"
                    size="icon"
                    className="h-7 w-7"
                    onClick={clearCompleted}
                    title={t("home.clearCompleted")}
                  >
                    <Trash2 size={14} />
                  </Button>
                </div>
              </div>
            </CardHeader>

            <CardContent className="flex-1 min-h-0 p-0">
              {activeTasks.length === 0 ? (
                <div className="flex h-full flex-col items-center justify-center p-6 text-center">
                  <Loader2
                    className="mx-auto mb-3 text-muted-foreground opacity-30"
                    size={32}
                  />
                  <p className="text-sm font-medium">{t("home.noTasks")}</p>
                </div>
              ) : (
                <ScrollArea className="h-full">
                  <div className="space-y-2 px-6 py-2 pr-3">
                    {activeTasks.slice(-10).reverse().map((task) => (
                      <TaskItem key={task.id} task={task} compact />
                    ))}
                  </div>
                </ScrollArea>
              )}
            </CardContent>

            <CardFooter className="mt-auto border-t border-border p-0">
              <div className="grid grid-cols-3 w-full">
                <div className="text-center py-3">
                  <div className="text-2xl font-bold tabular-nums">
                    {totalTasks}
                  </div>
                  <div className="text-xs text-muted-foreground uppercase tracking-wide">
                    {t("home.total")}
                  </div>
                </div>
                <div className="text-center py-3 border-l border-border">
                  <div className="text-2xl font-bold tabular-nums text-success">
                    {completedTasks}
                  </div>
                  <div className="text-xs text-muted-foreground uppercase tracking-wide">
                    {t("home.done")}
                  </div>
                </div>
                <div className="text-center py-3 border-l border-border">
                  <div className="text-2xl font-bold tabular-nums text-destructive">
                    {failedTasks}
                  </div>
                  <div className="text-xs text-muted-foreground uppercase tracking-wide">
                    {t("home.failed")}
                  </div>
                </div>
              </div>
            </CardFooter>
          </Card>
        </div>
        </div>
      </div>

      {converting && (
        <div className="px-6 py-2 bg-primary/5 border-t border-primary/20 flex items-center gap-2">
          <Loader2 className="animate-spin text-primary" size={16} />
          <span className="text-sm text-primary">{t("home.converting")}</span>
        </div>
      )}
    </div>
  );
}