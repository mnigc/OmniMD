import { useEffect, useState } from "react";
import {
  AlertCircle,
  FileText,
  FolderOpen,
  Inbox,
  ShieldAlert,
  Trash2,
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

type Page = "home" | "convert" | "history" | "settings";

interface HistoryPageProps {
  onNavigate?: (page: "home" | "convert") => void;
}

const statusBadgeVariant: Record<
  string,
  "default" | "secondary" | "destructive" | "outline" | "success" | "warning"
> = {
  Pending: "warning",
  Processing: "secondary",
  Completed: "success",
  Failed: "destructive",
  Cancelled: "secondary",
};

const statusKey: Record<string, string> = {
  Pending: "taskStatus.pending",
  Processing: "taskStatus.processing",
  Completed: "taskStatus.completed",
  Failed: "taskStatus.failed",
  Cancelled: "taskStatus.cancelled",
};

function formatTime(ts: number | null): string {
  if (!ts) return "\u2014";
  const d = new Date(ts);
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}`;
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

  const completed = (entry: HistoryEntry) => entry.status === "Completed";

  return (
    <div className="h-full flex flex-col">
      <div className="flex-1 min-h-0 overflow-auto p-6">
        <div className="max-w-4xl mx-auto flex flex-col gap-6">
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
                <Trash2 size={14} />
                {t("history.clearAll")}
              </Button>
            }
          />

          {actionError && (
            <div className="flex items-center gap-2 px-4 py-3 rounded-lg bg-destructive/10 border border-destructive/30 text-destructive text-sm">
              <AlertCircle size={16} className="shrink-0" />
              <span>{actionError}</span>
            </div>
          )}

          {history.length === 0 ? (
            <div className="text-center py-16">
              <Inbox
                className="mx-auto mb-4 text-muted-foreground opacity-30"
                size={48}
              />
              <p className="text-lg font-medium">{t("history.empty")}</p>
              <p className="text-sm text-muted-foreground mt-1">
                {t("history.emptyHint")}
              </p>
            </div>
          ) : (
            <div className="overflow-x-auto rounded-lg border border-border">
              <table className="w-full text-sm">
                <thead>
                  <tr className="border-b border-border bg-muted/40 text-left">
                    <th className="px-4 py-3 text-xs font-medium text-muted-foreground uppercase tracking-wide w-12">
                      #
                    </th>
                    <th className="px-4 py-3 text-xs font-medium text-muted-foreground uppercase tracking-wide">
                      {t("history.fileName")}
                    </th>
                    <th className="px-4 py-3 text-xs font-medium text-muted-foreground uppercase tracking-wide">
                      {t("history.outputMode")}
                    </th>
                    <th className="px-4 py-3 text-xs font-medium text-muted-foreground uppercase tracking-wide">
                      {t("history.time")}
                    </th>
                    <th className="px-4 py-3 text-xs font-medium text-muted-foreground uppercase tracking-wide w-24">
                      {t("history.status")}
                    </th>
                    <th className="px-4 py-3 text-xs font-medium text-muted-foreground uppercase tracking-wide text-right w-36">
                      {t("history.actions")}
                    </th>
                  </tr>
                </thead>
                <tbody>
                  {history.map((entry, index) => {
                    const fileName =
                      entry.sourcePath.split("/").pop()?.split("\\").pop() ||
                      entry.sourcePath;
                    return (
                      <tr
                        key={entry.id}
                        className="border-b border-border last:border-b-0 hover:bg-muted/40"
                      >
                        <td className="px-4 py-3 text-xs text-muted-foreground tabular-nums">
                          {index + 1}
                        </td>
                        <td className="px-4 py-3 min-w-0 max-w-xs">
                          <div className="flex items-center gap-2">
                            <FileText
                              size={14}
                              className="text-muted-foreground shrink-0"
                            />
                            <span
                              className={cn(
                                "truncate",
                                !completed(entry) && "text-destructive"
                              )}
                              title={entry.sourcePath}
                            >
                              {fileName}
                            </span>
                          </div>
                          {entry.error && (
                            <p className="flex items-center gap-1 text-xs text-destructive mt-0.5">
                              <ShieldAlert size={11} className="shrink-0" />
                              <Tooltip>
                                <TooltipTrigger asChild>
                                  <span className="line-clamp-2 break-words cursor-help">
                                    {entry.error}
                                  </span>
                                </TooltipTrigger>
                                <TooltipContent
                                  side="top"
                                  align="start"
                                  className="max-w-xs p-2 text-xs"
                                >
                                  <p className="whitespace-pre-wrap break-words">
                                    {entry.error}
                                  </p>
                                </TooltipContent>
                              </Tooltip>
                            </p>
                          )}
                        </td>
                        <td className="px-4 py-3">
                          <span className="text-xs px-1.5 py-0.5 bg-primary/10 text-primary rounded">
                            {t(`outputMode.${entry.outputMode}`)}
                          </span>
                        </td>
                        <td className="px-4 py-3 text-xs text-muted-foreground tabular-nums">
                          {formatTime(entry.completedAt ?? entry.createdAt)}
                        </td>
                        <td className="px-4 py-3">
                          <Badge
                            variant={statusBadgeVariant[entry.status] ?? "secondary"}
                          >
                            {t(statusKey[entry.status] || "taskStatus.pending")}
                          </Badge>
                        </td>
                        <td className="px-4 py-3">
                          <div className="flex items-center gap-1 justify-end">
                            <Button
                              variant="ghost"
                              size="icon"
                              className="h-7 w-7"
                              disabled={!completed(entry)}
                              onClick={() => handleOpenFile(entry)}
                              title={t("history.openFile")}
                            >
                              <FileText size={14} />
                            </Button>
                            <Button
                              variant="ghost"
                              size="icon"
                              className="h-7 w-7"
                              disabled={!completed(entry)}
                              onClick={() => handleOpenFolder(entry)}
                              title={t("history.openFolder")}
                            >
                              <FolderOpen size={14} />
                            </Button>
                            <Button
                              variant="ghost"
                              size="icon"
                              className="h-7 w-7"
                              onClick={() => handleDelete(entry)}
                              title={t("history.delete")}
                            >
                              <Trash2 size={14} />
                            </Button>
                          </div>
                        </td>
                      </tr>
                    );
                  })}
                </tbody>
              </table>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}


