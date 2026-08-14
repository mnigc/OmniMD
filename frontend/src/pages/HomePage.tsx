import { useCallback, useEffect, useState } from "react";
import {
  FilePlus,
  Download,
  FolderOpen,
  Loader2,
  Link,
  CheckCircle2,
  XCircle,
  Trash2,
  RotateCcw,
} from "lucide-react";
import { DropZone } from "../components/DropZone";
import { TaskItem } from "../components/TaskItem";
import { convertBatch, convertFile, getSupportedFormats } from "../api/tauriApi";
import { pickFiles, pickOutputDir } from "../api/dialogs";
import { useTaskStore } from "../store/useTaskStore";
type Page = "home" | "convert" | "batch" | "settings";

interface HomePageProps {
  onNavigate: (page: Page) => void;
}

export function HomePage({ onNavigate }: HomePageProps) {
  const {
    tasks,
    addTasks,
    finalizeTask,
    failTask,
    clearCompleted,
    setCurrentTask,
  } = useTaskStore();

  const [outputDir, setOutputDir] = useState("");
  const [concurrency, setConcurrency] = useState(4);
  const [converting, setConverting] = useState(false);
  const [supportedFormats, setSupportedFormats] = useState<string[]>([]);

  useEffect(() => {
    getSupportedFormats().then(setSupportedFormats).catch(() => {});
  }, []);

  const handleFiles = useCallback(
    async (paths: string[]) => {
      if (paths.length === 0) return;

      const dir = outputDir || paths[0].replace(/\\/g, "/").split("/").slice(0, -1).join("/");
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
    [addTasks, finalizeTask, failTask, setCurrentTask, onNavigate, outputDir, concurrency]
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
  const completedTasks = tasks.filter((t) => t.status === "Completed").length;
  const failedTasks = tasks.filter((t) => t.status === "Failed").length;

  const activeTasks = tasks.filter(
    (t) => t.status !== "Completed" && t.status !== "Cancelled"
  );

  return (
    <div className="h-full flex flex-col">
      <div className="flex-1 p-6 flex flex-col gap-6 overflow-auto">
        <div className="text-center mb-2">
          <h1 className="text-3xl font-bold text-slate-900">
            Convert Anything to Markdown
          </h1>
          <p className="text-muted-foreground mt-2 text-sm">
            Drop DOCX, PDF, PPTX, XLSX, EPUB, CSV, TXT, HTML, and more
          </p>
        </div>

        <div className="grid grid-cols-1 lg:grid-cols-3 gap-6 flex-1">
          <div className="lg:col-span-2 flex flex-col gap-4">
            <div className="flex items-center gap-3 p-3 bg-slate-50 rounded-lg border border-border">
              <input
                type="text"
                value={outputDir}
                onChange={(e) => setOutputDir(e.target.value)}
                placeholder="Output directory (e.g. /home/user/output)"
                className="flex-1 px-3 py-1.5 text-sm border border-slate-200 rounded-md focus:outline-none focus:ring-2 focus:ring-violet-500"
              />
              <button
                onClick={handleBrowseOutputDir}
                className="flex items-center gap-1.5 px-3 py-1.5 bg-slate-900 text-white text-sm rounded-md hover:bg-slate-800 transition-colors"
              >
                <FolderOpen size={14} />
                Browse
              </button>
            </div>

            <DropZone
              onFiles={handleFiles}
              disabled={converting}
              formats={supportedFormats}
            />

            <div className="flex items-center gap-3">
              <button
                onClick={handleAddFiles}
                disabled={converting}
                className="flex items-center gap-1.5 px-4 py-2 bg-violet-600 text-white text-sm rounded-md hover:bg-violet-700 disabled:opacity-50 transition-colors"
              >
                <FilePlus size={16} />
                Add Files
              </button>
              <div className="flex items-center gap-2 px-3 py-1.5 border border-slate-200 rounded-md">
                <Link size={14} className="text-muted-foreground" />
                <input
                  type="text"
                  placeholder="Paste URL (Phase 2)"
                  disabled
                  className="text-xs text-muted-foreground bg-transparent focus:outline-none w-40"
                />
              </div>
              <div className="ml-auto flex items-center gap-2">
                <label className="text-xs text-muted-foreground">
                  Concurrency
                </label>
                <input
                  type="number"
                  min={1}
                  max={16}
                  value={concurrency}
                  onChange={(e) =>
                    setConcurrency(parseInt(e.target.value) || 1)
                  }
                  className="w-14 px-2 py-1 text-sm border border-slate-200 rounded-md focus:outline-none focus:ring-2 focus:ring-violet-500"
                />
              </div>
            </div>
          </div>

          <div className="bg-slate-50 rounded-lg border border-border p-4 flex flex-col">
            <div className="flex items-center justify-between mb-3">
              <h3 className="text-sm font-semibold text-slate-900">
                Recent Conversions
              </h3>
              <div className="flex items-center gap-1">
                <button
                  onClick={() => onNavigate("batch")}
                  className="p-1 rounded hover:bg-slate-200 transition-colors"
                  title="View all"
                >
                  <RotateCcw size={14} />
                </button>
                <button
                  onClick={clearCompleted}
                  className="p-1 rounded hover:bg-slate-200 transition-colors"
                  title="Clear completed"
                >
                  <Trash2 size={14} />
                </button>
              </div>
            </div>

            <div className="flex-1 overflow-auto space-y-2">
              {activeTasks.slice(-10).reverse().map((task) => (
                <TaskItem key={task.id} task={task} compact />
              ))}
              {activeTasks.length === 0 && (
                <p className="text-xs text-center text-muted-foreground py-8">
                  No tasks yet
                </p>
              )}
            </div>

            <div className="mt-3 pt-3 border-t border-border grid grid-cols-3 gap-2">
              <div className="text-center">
                <div className="text-lg font-bold text-slate-900">
                  {totalTasks}
                </div>
                <div className="text-[10px] text-muted-foreground uppercase tracking-wide">
                  Total
                </div>
              </div>
              <div className="text-center">
                <div className="text-lg font-bold text-green-600">
                  {completedTasks}
                </div>
                <div className="text-[10px] text-muted-foreground uppercase tracking-wide">
                  Done
                </div>
              </div>
              <div className="text-center">
                <div className="text-lg font-bold text-red-600">
                  {tasks.filter((t) => t.status === "Failed").length}
                </div>
                <div className="text-[10px] text-muted-foreground uppercase tracking-wide">
                  Failed
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>

      {converting && (
        <div className="px-6 py-2 bg-violet-50 border-t border-violet-200 flex items-center gap-2">
          <Loader2 className="animate-spin text-violet-600" size={16} />
          <span className="text-sm text-violet-700">Converting...</span>
        </div>
      )}
    </div>
  );
}
