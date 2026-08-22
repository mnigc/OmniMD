import { useEffect, useState } from "react";
import {
  AlertCircle,
  FileText,
  FolderOpen,
  Inbox,
  ShieldAlert,
  Trash2,
  Clock,
  Sparkles,
} from "lucide-react";
import { PageHeader } from "../components/PageHeader";
import { Badge } from "../components/ui/badge";
import { Button } from "../components/ui/button";
import { useTaskStore } from "../store/useTaskStore";
import { useI18n } from "../i18n";
import { readTextFile, openFolder } from "../api/tauriApi";
import { dirname, resolve } from "@tauri-apps/api/path";
import type {
  ConversionResult,
  ConversionTask,
  HistoryEntry,
} from "../types";
import { cn } from "../lib/utils";
import { deriveStatsFromMarkdown } from "../lib/stats";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "../components/ui/tooltip";

const statusBadgeVariant: Record<
  string,
  "default" | "secondary" | "destructive" | "outline" | "success" | "warning"
> = {
  Pending: "warning",
  Processing: "secondary",
  Completed: "success",
  Failed: "destructive",
  Canceled: "secondary",
  Cancelled: "secondary",
};

const statusKey: Record<string, string> = {
  Pending: "taskStatus.pending",
  Processing: "taskStatus.processing",
  Completed: "taskStatus.completed",
  Failed: "taskStatus.failed",
  Cancelled: "taskStatus.cancelled",
};

const modeColors: Record<string, string> = {
  standard: "bg-muted/60 text-muted-foreground",
  aiReady: "bg-primary/10 text-primary",
  obsidian: "bg-violet-500/10 text-violet-600 dark:text-violet-300",
};

function formatTime(ts: number | null): string {
  if (!ts) return "\u2014";
  const d = new Date(ts);
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}`;
}

interface HistoryPageProps {
  onNavigate?: (page: "home" | "convert") => void;
}

function HistoryCard({
  entry,
  index,
  onOpenFile,
  onOpenFolder,
  onDelete,
}: {
  entry: HistoryEntry;
  index: number;
  onOpenFile: () => void;
  onOpenFolder: () => void;
  onDelete: () => void;
}) {
  const { t } = useI18n();
  const fileName =
    entry.sourcePath.split("/").pop()?.split("\\").pop() || entry.sourcePath;
  const completed = entry.status === "Completed";

  return (
    <div
      className={cn(
        "group flex items-center gap-4 p-3.5 rounded-xl border transition-all duration-200",
        "border-border/70 bg-background",
        "hover:border-primary/30 hover:shadow-sm hover:shadow-primary/5",
        entry.status === "Failed" && "border-destructive/20 bg-destructive/3"
      )}
    >
      <div className="flex items-center justify-center w-8 h-8 rounded-lg bg-muted/60 shrink-0">
        <FileText size={14} className="text-muted-foreground" />
      </div>

      <div className="flex-1 min-w-0">
        <div className="flex items-center gap-2">
          <span
            className={cn(
              "font-medium text-sm truncate cursor-help",
              !completed && "text-destructive"
            )}
            title={entry.sourcePath}
          >
            {fileName}
          </span>
          <span
            className={cn(
              "text-[10px] px-1.5 py-0.5 rounded font-medium shrink-0",
              modeColors[entry.outputMode] || modeColors.standard
            )}
          >
            {t(`outputMode.${entry.outputMode}`)}
          </span>
        </div>
        {entry.error && (
          <Tooltip>
            <TooltipTrigger asChild>
              <p className="flex items-center gap-1 text-xs text-destructive mt-0.5 line-clamp-1 break-words cursor-help">
                <ShieldAlert size={10} className="shrink-0" />
                {entry.error}
              </p>
            </TooltipTrigger>
            <TooltipContent
              side="bottom"
              align="start"
              className="max-w-xs p-2 text-xs"
            >
              <p className="whitespace-pre-wrap break-words">
                {entry.error}
              </p>
            </TooltipContent>
          </Tooltip>
        )}
      </div>

      <div className="hidden sm:flex items-center gap-3 shrink-0">
        <div className="flex items-center gap-1.5 text-xs text-muted-foreground">
          <Clock size={11} />
          <span className="tabular-nums">{formatTime(entry.completedAt ?? entry.createdAt)}</span>
        </div>
        <Badge variant={statusBadgeVariant[entry.status] ?? "secondary"} className="text-[10px] px-2">
          {t(statusKey[entry.status] || "taskStatus.pending")}
        </Badge>
      </div>

      <div className="flex items-center gap-0.5 shrink-0">
        <Button
          variant="ghost"
          size="icon"
          className="h-7 w-7 opacity-0 group-hover:opacity-100 transition-opacity"
          disabled={!completed}
          onClick={onOpenFile}
          title={t("history.openFile")}
        >
          <FileText size={13} />
        </Button>
        <Button
          variant="ghost"
          size="icon"
          className="h-7 w-7 opacity-0 group-hover:opacity-100 transition-opacity"
          disabled={!completed}
          onClick={onOpenFolder}
          title={t("history.openFolder")}
        >
          <FolderOpen size={13} />
        </Button>
        <Button
          variant="ghost"
          size="icon"
          className="h-7 w-7 opacity-0 group-hover:opacity-100 transition-opacity"
          onClick={onDelete}
          title={t("history.delete")}
        >
          <Trash2 size={13} />
        </Button>
      </div>
    </div>
  );
}

export function HistoryPage({ onNavigate }: HistoryPageProps) {
  const { t } = useI18n();
  const { history, clearHistoryItem, clearAllHistory, setCurrentTask } =
    useTaskStore();

  const [actionError, setActionError] = useState("");

  useEffect(() => {
    if (!actionError) return;
    const timer = setTimeout(() => setActionError(""), 4000);
    return () => clearTimeout(timer);
  }, [actionError]);

  const handleOpenFile = async (entry: HistoryEntry) => {
    try {
      const content = await readTextFile(entry.outputPath);
      const stats = deriveStatsFromMarkdown(content);
      const task: ConversionTask = {
        id: entry.id,
        sourcePath: entry.sourcePath,
        outputDir: entry.outputPath.split(/[\\/]/).slice(0, -1).join("/") || ".",
        outputPath: entry.outputPath,
        outputMode: entry.outputMode,
        status: "Completed",
        progress: 1,
        stage: "Saving",
        error: null,
        createdAt: entry.createdAt,
        completedAt: entry.completedAt,
      };
      const result: ConversionResult = {
        taskId: entry.id,
        markdown: content,
        documentSerialized: "",
        assetCount: 0,
        errors: [],
        success: true,
        outputPath: entry.outputPath,
        stats,
      };
      setCurrentTask(task, result, "history");
      onNavigate?.("convert");
    } catch (err: any) {
      setActionError(t("history.openFileError") + ": " + (err?.message ?? err));
    }
  };

  const handleOpenFolder = async (entry: HistoryEntry) => {
    try {
      let dir = await dirname(entry.outputPath);
      dir = await resolve(dir);
      if (!dir) return;
      await openFolder(dir);
      setActionError("");
    } catch {
      setActionError(t("history.openFolderError"));
    }
  };

  const handleDelete = (entry: HistoryEntry) => {
    clearHistoryItem(entry.id);
  };

  const handleClearAll = () => {
    if (window.confirm(t("history.clearConfirm"))) {
      clearAllHistory();
    }
  };

  return (
    <div className="h-full flex flex-col">
      <div className="flex-1 min-h-0 overflow-auto p-6">
        <div className="max-w-3xl mx-auto flex flex-col gap-5">
          <PageHeader
            title={t("history.title")}
            description={
              history.length > 0 ? undefined : t("history.emptyHint")
            }
            actions={
              <Button
                variant="outline"
                size="sm"
                onClick={handleClearAll}
                disabled={history.length === 0}
                title={t("history.clearAll")}
              >
                <Trash2 size={13} />
                {t("history.clearAll")}
              </Button>
            }
          />

          {actionError && (
            <div className="flex items-center gap-2 px-4 py-3 rounded-lg bg-destructive/10 border border-destructive/30 text-destructive text-sm">
              <AlertCircle size={14} className="shrink-0" />
              <span>{actionError}</span>
            </div>
          )}

          {history.length === 0 ? (
            <div className="text-center py-20">
              <div className="w-14 h-14 mx-auto mb-4 rounded-2xl bg-muted/60 flex items-center justify-center">
                <Inbox size={24} className="text-muted-foreground" />
              </div>
              <p className="text-lg font-medium">{t("history.empty")}</p>
              <p className="text-sm text-muted-foreground mt-1">
                {t("history.emptyHint")}
              </p>
            </div>
          ) : (
            <div className="flex flex-col gap-2">
              {history.map((entry, index) => (
                <HistoryCard
                  key={entry.id}
                  entry={entry}
                  index={index}
                  onOpenFile={() => handleOpenFile(entry)}
                  onOpenFolder={() => handleOpenFolder(entry)}
                  onDelete={() => handleDelete(entry)}
                />
              ))}
            </div>
          )}
        </div>
      </div>
    </div>
  );
}