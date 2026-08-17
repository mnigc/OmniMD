import { useEffect, useState } from "react";
import {
  X,
  Play,
  Loader2,
  Inbox,
} from "lucide-react";
import { useBatchStore } from "../store/useBatchStore";
import { useI18n } from "../i18n";
import { Button } from "./ui/button";
import { ScrollArea } from "./ui/scroll-area";
import { BatchTaskItem } from "./BatchTaskItem";
import { BatchTaskFilterBar } from "./BatchTaskFilterBar";
import { BatchTaskToolbar } from "./BatchTaskToolbar";

interface BatchTaskPanelProps {
  open: boolean;
  onClose: () => void;
}

export function BatchTaskPanel({ open, onClose }: BatchTaskPanelProps) {
  const { t } = useI18n();
  const { tasks, summary, start, loading, refreshTasks, refreshSummary, listenForEvents } = useBatchStore();
  const [filter, setFilter] = useState<string>("all");

  useEffect(() => {
    if (open) {
      refreshTasks();
      refreshSummary();
    }
  }, [open, refreshTasks, refreshSummary]);

  useEffect(() => {
    let cleanup: (() => void) | null = null;
    if (open) {
      listenForEvents().then((fn) => { cleanup = fn; });
    }
    return () => { cleanup?.(); };
  }, [open, listenForEvents]);

  const filteredTasks = filter === "all"
    ? tasks
    : tasks.filter((t) => t.status.toLowerCase() === filter);

  if (!open) return null;

  return (
    <>
      <div
        className="fixed inset-0 bg-black/40 z-40 transition-opacity"
        onClick={onClose}
      />
      <div className="fixed right-0 top-0 h-full w-[480px] max-w-[90vw] bg-background border-l border-border z-50 shadow-2xl flex flex-col"
        style={{ animation: "slideIn 0.25s ease-out" }}
      >
        <style>{`
          @keyframes slideIn {
            from { transform: translateX(100%); }
            to { transform: translateX(0); }
          }
        `}</style>
        <div className="flex items-center justify-between px-5 py-4 border-b border-border shrink-0">
          <h2 className="text-sm font-semibold">{t("batch.title")}</h2>
          <Button variant="ghost" size="icon" onClick={onClose}>
            <X size={16} />
          </Button>
        </div>

        <BatchTaskFilterBar
          current={filter}
          onChange={setFilter}
          summary={summary}
        />

        <BatchTaskToolbar />

        <ScrollArea className="flex-1 px-3 py-2">
          {filteredTasks.length === 0 ? (
            <div className="flex flex-col items-center justify-center py-12 text-center">
              <Inbox className="mx-auto mb-3 text-muted-foreground opacity-30" size={32} />
              <p className="text-sm font-medium">{t("batch.noTasks")}</p>
              <p className="text-xs text-muted-foreground mt-1">{t("batch.noTasksHint")}</p>
            </div>
          ) : (
            <div className="space-y-2">
              {filteredTasks.map((task) => (
                <BatchTaskItem key={task.id} task={task} />
              ))}
            </div>
          )}
        </ScrollArea>

        {tasks.some((t) => t.status === "Pending") && (
          <div className="border-t border-border p-3 shrink-0">
            <Button
              className="w-full"
              onClick={start}
              disabled={loading}
            >
              {loading ? (
                <Loader2 size={16} className="animate-spin mr-2" />
              ) : (
                <Play size={16} className="mr-2" />
              )}
              {t("home.startConversion")}
            </Button>
          </div>
        )}
      </div>
    </>
  );
}