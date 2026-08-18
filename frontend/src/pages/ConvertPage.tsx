import { useEffect, useRef, useState } from "react";
import {
  Copy,
  FileDown,
  FolderOpen,
  LayoutTemplate,
  Code,
  Eye,
  RefreshCw,
  ArrowLeft,
  Loader2,
  AlertCircle,
  Image as ImageIcon,
  Save,
  Table as TableIcon,
  Type,
} from "lucide-react";
import { EditorView } from "@codemirror/view";
import { MarkdownPreview } from "../components/MarkdownPreview";
import { MarkdownEditor } from "../components/MarkdownEditor";
import { EditorToolbar } from "../components/EditorToolbar";
import { SegmentedControl } from "../components/SegmentedControl";
import { useAutoSave } from "../hooks/useAutoSave";
import { useTaskStore } from "../store/useTaskStore";
import { useSettingsStore } from "../store/useSettingsStore";
import { useI18n } from "../i18n";
import { Button } from "../components/ui/button";
import { Separator } from "../components/ui/separator";
import { convertFile, writeTextFile, openFolder } from "../api/tauriApi";
import type { ConversionStats, ErrorDto } from "../types";
import { save } from "@tauri-apps/plugin-dialog";
import { dirname, resolve } from "@tauri-apps/api/path";
import { showToast } from "../lib/toast";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "../components/ui/tooltip";

type ViewMode = "source" | "preview" | "split";

const SPLIT_RATIO_KEY = "omnimd_split_ratio";
const MIN_SPLIT_RATIO = 0.2;
const MAX_SPLIT_RATIO = 0.8;

function suggestForCode(t: (path: string) => string, code: string): string {
  const key = `error.suggest.${code}`;
  const val = t(key);
  return val === key ? t("convert.errorCard.suggestionUnavailable") : val;
}

function markdownToPlainText(md: string): string {
  const lines = md.split("\n");
  let inCode = false;
  const out: string[] = [];
  for (const raw of lines) {
    if (raw.trimStart().startsWith("```")) {
      inCode = !inCode;
      out.push(raw);
      continue;
    }
    if (inCode) {
      out.push(raw);
      continue;
    }
    let line = raw;
    line = line.replace(/^#{1,6}\s*/, "");
    line = line.replace(/^>\s?/, "");
    line = line.replace(/^[-*+]\s+/, "");
    line = line.replace(/^\d+\.\s+/, "");
    line = line.replace(/!\[([^\]]*)\]\([^)]*\)/g, "$1");
    line = line.replace(/\[([^\]]*)\]\([^)]*\)/g, "$1");
    line = line.replace(/\*\*([^*]+)\*\*/g, "$1");
    line = line.replace(/\*([^*]+)\*/g, "$1");
    line = line.replace(/_([^_]+)_/g, "$1");
    line = line.replace(/`([^`]+)`/g, "$1");
    if (line.trim().startsWith("|")) {
      line = line
        .split("|")
        .map((c) => c.trim())
        .filter(Boolean)
        .join("  ");
    }
    out.push(line);
  }
  return out.join("\n");
}

function Stat({
  icon,
  label,
  value,
}: {
  icon: React.ReactNode;
  label: string;
  value: string;
}) {
  return (
    <div className="flex items-center gap-1.5" title={`${label}: ${value}`}>
      <span className="text-muted-foreground/80">{icon}</span>
      <span className="text-xs text-muted-foreground">{label}</span>
      <span className="text-xs font-medium tabular-nums">{value}</span>
    </div>
  );
}

function ErrorCard({
  fileName,
  errors,
  fallbackError,
}: {
  fileName: string;
  errors: ErrorDto[];
  fallbackError?: string | null;
}) {
  const { t } = useI18n();
  if (errors.length === 0 && !fallbackError) return null;
  const first = errors[0];
  const message = first?.message || fallbackError || "";
  const code = first?.code || "";
  const suggestion = first ? suggestForCode(t, code) : "";

  return (
    <div className="mx-4 mb-2 rounded-md border border-destructive/40 bg-destructive/5 p-3">
      <div className="flex items-start gap-2">
        <AlertCircle
          size={16}
          className="text-destructive mt-0.5 shrink-0"
        />
        <div className="flex-1 min-w-0">
          <Tooltip>
            <TooltipTrigger asChild>
              <div className="text-sm font-medium text-destructive truncate cursor-help">
                {fileName}
              </div>
            </TooltipTrigger>
            <TooltipContent
              side="bottom"
              align="start"
              className="max-w-xs text-xs"
            >
              {fileName}
            </TooltipContent>
          </Tooltip>
          <div className="mt-1 text-xs text-muted-foreground">
            <span className="text-muted-foreground/80">
              {t("convert.errorCard.reason")}:
            </span>{" "}
            <Tooltip>
              <TooltipTrigger asChild>
                <span className="text-foreground cursor-help line-clamp-2 break-words">
                  {message}
                </span>
              </TooltipTrigger>
              <TooltipContent
                side="bottom"
                align="start"
                className="max-w-xs p-2 text-xs"
              >
                <p className="whitespace-pre-wrap break-words">{message}</p>
              </TooltipContent>
            </Tooltip>
          </div>
          {suggestion && (
            <div className="mt-1 text-xs">
              <span className="text-muted-foreground/80">
                {t("convert.errorCard.suggestion")}:
              </span>
              {" "}
              <Tooltip>
                <TooltipTrigger asChild>
                  <span className="line-clamp-1 cursor-help">
                    {suggestion}
                  </span>
                </TooltipTrigger>
                <TooltipContent
                  side="bottom"
                  align="start"
                  className="max-w-xs p-2 text-xs"
                >
                  <p className="whitespace-pre-wrap break-words">{suggestion}</p>
                </TooltipContent>
              </Tooltip>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}

function SplitDivider({
  onResize,
}: {
  onResize: (ratio: number) => void;
}) {
  const containerRef = useRef<HTMLDivElement>(null);
  const dragging = useRef(false);
  const startRatio = useRef(0.5);
  const containerWidth = useRef(1000);

  const handlePointerDown = (e: React.PointerEvent) => {
    e.preventDefault();
    dragging.current = true;
    const rect = containerRef.current?.parentElement?.getBoundingClientRect();
    if (rect) containerWidth.current = rect.width;
    const ratio = parseFloat(
      document.body.style.getPropertyValue("--omnimd-split-ratio") || "0.5",
    );
    startRatio.current = ratio;
    (e.target as HTMLElement).setPointerCapture(e.pointerId);
  };

  useEffect(() => {
    const handleMove = (e: PointerEvent) => {
      if (!dragging.current) return;
      e.preventDefault();
      const rect = containerRef.current?.parentElement?.getBoundingClientRect();
      if (!rect) return;
      const containerW = rect.width;
      const offsetX = e.clientX - rect.left;
      const ratio = Math.max(
        MIN_SPLIT_RATIO,
        Math.min(MAX_SPLIT_RATIO, offsetX / containerW),
      );
      onResize(ratio);
    };

    const handleUp = () => {
      dragging.current = false;
    };

    document.addEventListener("pointermove", handleMove, { passive: false });
    document.addEventListener("pointerup", handleUp);
    return () => {
      document.removeEventListener("pointermove", handleMove);
      document.removeEventListener("pointerup", handleUp);
    };
  }, [onResize]);

  return (
    <div
      ref={containerRef}
      className="w-[6px] shrink-0 cursor-col-resize flex items-center justify-center hover:bg-primary/10 transition-colors group"
      onPointerDown={handlePointerDown}
    >
      <div className="w-0.5 h-8 rounded-full bg-border group-hover:bg-primary/40 transition-colors" />
    </div>
  );
}

interface ConvertPageProps {
  onNavigate?: (page: "home" | "convert" | "history" | "settings") => void;
}

export function ConvertPage({ onNavigate }: ConvertPageProps) {
  const { t } = useI18n();
  const {
    currentTask,
    currentResult,
    setCurrentTask,
    previewSource,
  } = useTaskStore();
  const { outputMode, buildAiReadyOpts } = useSettingsStore();
  const [viewMode, setViewMode] = useState<ViewMode>("split");
  const [markdown, setMarkdown] = useState("");
  const [reconverting, setReconverting] = useState(false);
  const editorRef = useRef<EditorView | null>(null);

  const isHistoryMode = previewSource === "history";

  const storedRatio =
    parseFloat(localStorage.getItem(SPLIT_RATIO_KEY) ?? "0.5") || 0.5;
  const [splitRatio, setSplitRatio] = useState(
    () => Math.max(MIN_SPLIT_RATIO, Math.min(MAX_SPLIT_RATIO, storedRatio)),
  );

  const handleSplitChange = (ratio: number) => {
    setSplitRatio(ratio);
    localStorage.setItem(SPLIT_RATIO_KEY, String(ratio));
  };

  const autoSavePath = currentResult?.outputPath || currentTask?.outputPath || null;
  const { saving, saveNow } = useAutoSave(markdown, autoSavePath);

  const stats: ConversionStats | undefined = currentResult?.stats;
  const fileName =
    currentTask?.sourcePath.split(/[\\/]/).pop() || currentTask?.sourcePath || "";
  const hasError =
    (currentResult?.errors && currentResult.errors.length > 0) || !!currentTask?.error;

  useEffect(() => {
    if (currentResult?.markdown) {
      setMarkdown(currentResult.markdown);
    }
  }, [currentResult]);

  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key === "s") {
        e.preventDefault();
        saveNow();
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [saveNow]);

  const viewOptions: {
    value: ViewMode;
    icon: React.ReactNode;
    label: string;
  }[] = [
    { value: "source", icon: <Code size={16} />, label: t("editor.sourceMode") },
    {
      value: "preview",
      icon: <Eye size={16} />,
      label: t("convert.previewOnly"),
    },
    {
      value: "split",
      icon: <LayoutTemplate size={16} />,
      label: t("convert.splitView"),
    },
  ];

  if (!currentTask || !currentResult) {
    return (
      <div className="h-full flex items-center justify-center">
        <div className="text-center">
          <FileDown
            className="mx-auto mb-4 text-muted-foreground opacity-50"
            size={48}
          />
          <p className="text-lg font-medium">{t("convert.noFile")}</p>
          <p className="text-sm text-muted-foreground mt-1">
            {t("convert.noFileHint")}
          </p>
          {onNavigate && (
            <Button
              variant="outline"
              size="sm"
              className="mt-4"
              onClick={() => onNavigate("history")}
            >
              <ArrowLeft size={14} />
              {t("convert.openHistory")}
            </Button>
          )}
        </div>
      </div>
    );
  }

  return (
    <div className="h-full flex flex-col">
      <div className="border-b border-border px-4 py-2 flex items-center gap-3 shrink-0">
        <div className="flex items-center gap-2 h-8 px-3 bg-muted rounded-md">
          <FileDown size={14} className="text-muted-foreground" />
          <Tooltip>
            <TooltipTrigger asChild>
              <span className="text-sm font-medium truncate max-w-[480px] cursor-help">
                {currentTask.sourcePath.split("/").pop()}
              </span>
            </TooltipTrigger>
            <TooltipContent side="bottom" align="start" className="max-w-xs text-xs">
              {currentTask.sourcePath}
            </TooltipContent>
          </Tooltip>
        </div>

        <div className="flex items-center gap-2 ml-auto">
          {autoSavePath && (
            <span className="text-xs text-muted-foreground">
              {saving ? t("editor.saving") : t("editor.saved")}
            </span>
          )}
          <SegmentedControl
            options={viewOptions}
            value={viewMode}
            onChange={setViewMode}
            size="md"
          />
        </div>
      </div>

      <div className="flex-1 flex overflow-hidden">
        {(viewMode === "source" || viewMode === "split") && (
          <div
            className={`flex flex-col min-w-0 ${viewMode === "split" ? "border-r border-border" : ""}`}
            style={
              viewMode === "split"
                ? { flex: "0 0 auto", width: `${splitRatio * 100}%` }
                : { flex: "1 1 0%" }
            }
          >
            <EditorToolbar view={editorRef.current} />
            <div className="flex-1 overflow-hidden">
              <MarkdownEditor
                value={markdown}
                onChange={setMarkdown}
                onViewReady={(v) => { editorRef.current = v; }}
              />
            </div>
          </div>
        )}

        {viewMode === "split" && (
          <SplitDivider onResize={handleSplitChange} />
        )}

        {(viewMode === "preview" || viewMode === "split") && (
          <div
            className="flex flex-col min-w-0"
            style={
              viewMode === "split"
                ? { flex: "0 0 auto", width: `${(1 - splitRatio) * 100}%` }
                : { flex: "1 1 0%" }
            }
          >
            <div className="h-9 border-b border-border px-3 flex items-center gap-2 bg-muted/30">
              <span className="text-xs font-medium text-muted-foreground">
                {t("convert.preview")}
              </span>
              <span className="text-xs text-muted-foreground ml-auto">
                {currentResult.assetCount} {t("convert.assets")}
              </span>
            </div>
            <div className="flex-1 overflow-auto p-4 prose prose-sm max-w-none">
              <MarkdownPreview content={markdown} />
            </div>
          </div>
        )}
      </div>

      {hasError && (
        <ErrorCard
          fileName={fileName}
          errors={currentResult?.errors ?? []}
          fallbackError={currentTask?.error}
        />
      )}

      {stats && !hasError && (
        <div className="border-t border-border bg-muted/20 px-4 py-2 flex items-center gap-4 shrink-0 overflow-x-auto">
          <Stat
            icon={<ImageIcon size={13} />}
            label={t("convert.stats.images")}
            value={String(stats.imageCount)}
          />
          <Stat
            icon={<TableIcon size={13} />}
            label={t("convert.stats.tables")}
            value={String(stats.tableCount)}
          />
          <Stat
            icon={<Type size={13} />}
            label={t("convert.stats.words")}
            value={String(stats.wordCount ?? 0)}
          />
        </div>
      )}

      <div className="border-t border-border bg-muted/30 px-4 py-3 flex items-center gap-2 shrink-0">
        <Button
          variant="outline"
          size="sm"
          onClick={async () => {
            if (!navigator.clipboard) return;
            try {
              await navigator.clipboard.writeText(markdown);
              showToast(t("toast.copied"), 2000);
            } catch {
              // ignore
            }
          }}
        >
          <Copy size={14} />
          {t("convert.copyMarkdown")}
        </Button>

        {!isHistoryMode && (
          <Button
            variant="outline"
            size="sm"
            onClick={async () => {
              if (!navigator.clipboard) return;
              try {
                await navigator.clipboard.writeText(markdownToPlainText(markdown));
                showToast(t("toast.copied"), 2000);
              } catch {
                // ignore
              }
            }}
          >
            <Copy size={14} />
            {t("convert.copyPlainText")}
          </Button>
        )}

        {!isHistoryMode && (
          <Button
            variant="outline"
            size="sm"
            onClick={async () => {
              if (!currentTask) return;
              const sourceName =
                currentTask.sourcePath.split(/[\\/]/).pop() || "output";
              const defaultName = sourceName.replace(/\.[^.]+$/, ".md");
              try {
                const filePath = await save({
                  defaultPath: defaultName,
                  filters: [{ name: "Markdown", extensions: ["md"] }],
                });
                if (filePath) {
                  await writeTextFile(filePath, markdown);
                }
              } catch {
                // user cancelled
              }
            }}
          >
            <FileDown size={14} />
            {t("convert.saveMd")}
          </Button>
        )}

        <Button
          variant="outline"
          size="sm"
          onClick={async () => {
            if (!currentTask) return;
            try {
              const dir = await resolve(await dirname(currentTask.outputPath));
              if (!dir) return;
              await openFolder(dir);
            } catch {
              // ignore
            }
          }}
        >
          <FolderOpen size={14} />
          {t("convert.openFolder")}
        </Button>

        {!isHistoryMode && (
          <Button
            variant="outline"
            size="sm"
            disabled={reconverting}
            onClick={async () => {
              if (!currentTask || reconverting) return;
              setReconverting(true);
              try {
                const sourcePath = currentTask.sourcePath;
                const dir = await resolve(await dirname(currentTask.outputPath));
                const result = await convertFile(
                  sourcePath,
                  dir,
                  outputMode,
                  buildAiReadyOpts(),
                );
                const fileName = sourcePath.split(/[\\/]/).pop() || "output";
                const outputName = fileName.replace(/\.[^.]+$/, ".md");
                const outputPath = `${dir}/${outputName}`;
                setCurrentTask(
                  {
                    id: result.taskId,
                    sourcePath: sourcePath,
                    outputDir: dir,
                    outputPath,
                    outputMode,
                    status: "Completed",
                    progress: 1,
                    stage: "Saving",
                    error: null,
                    createdAt: Date.now(),
                    completedAt: Date.now(),
                  },
                  result,
                );
              } catch (err: any) {
                // ignore
              } finally {
                setReconverting(false);
              }
            }}
          >
            {reconverting ? (
              <Loader2 size={14} className="animate-spin" />
            ) : (
              <RefreshCw size={14} />
            )}
            {t("convert.reconvert")}
          </Button>
        )}

        <Separator orientation="vertical" className="h-6 mx-1" />

        <Tooltip>
          <TooltipTrigger asChild>
            <Button
              className="ml-auto"
              variant="outline"
              size="icon"
              onClick={() => {
                showToast(
                  `${t("toast.aiReadyTip")}`,
                  4000,
                );
              }}
            >
              <span className="text-xs font-bold">?</span>
            </Button>
          </TooltipTrigger>
          <TooltipContent side="top" align="end" className="max-w-xs text-xs">
            <p>{t("toast.aiReadyTip")}</p>
          </TooltipContent>
        </Tooltip>
      </div>
    </div>
  );
}