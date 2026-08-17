import { RotateCcw, Trash2, X } from "lucide-react";
import { useBatchStore } from "../store/useBatchStore";
import { useI18n } from "../i18n";
import { Button } from "./ui/button";
import { Label } from "./ui/label";

export function BatchTaskToolbar() {
  const { t } = useI18n();
  const { tasks, concurrency, setConcurrency, cancelAll, retryFailed, clearDone } = useBatchStore();

  const hasFailed = tasks.some((t) => t.status === "Failed");
  const hasDone = tasks.some((t) => t.status === "Completed" || t.status === "Cancelled");
  const hasActive = tasks.some((t) => t.status === "Pending" || t.status === "Processing");

  return (
    <div className="flex items-center gap-2 px-3 py-2 border-b border-border shrink-0">
      <div className="flex items-center gap-1.5">
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

      <div className="ml-auto flex items-center gap-1">
        {hasActive && (
          <Button
            variant="ghost"
            size="sm"
            className="h-7 text-xs"
            onClick={cancelAll}
          >
            <X size={12} className="mr-1" />
            {t("batch.cancelAll")}
          </Button>
        )}
        {hasFailed && (
          <Button
            variant="ghost"
            size="sm"
            className="h-7 text-xs"
            onClick={retryFailed}
          >
            <RotateCcw size={12} className="mr-1" />
            {t("batch.retryFailed")}
          </Button>
        )}
        {hasDone && (
          <Button
            variant="ghost"
            size="sm"
            className="h-7 text-xs"
            onClick={clearDone}
          >
            <Trash2 size={12} className="mr-1" />
            {t("batch.clearDone")}
          </Button>
        )}
      </div>
    </div>
  );
}