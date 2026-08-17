import { useI18n } from "../i18n";
import { cn } from "../lib/utils";
import type { BatchSummaryDto } from "../types";

interface BatchTaskFilterBarProps {
  current: string;
  onChange: (filter: string) => void;
  summary: BatchSummaryDto | null;
}

const filters = [
  { key: "all", labelKey: "batch.total", countKey: "total" as const },
  { key: "processing", labelKey: "batch.processing", countKey: "processing" as const },
  { key: "completed", labelKey: "batch.completed", countKey: "completed" as const },
  { key: "failed", labelKey: "batch.failed", countKey: "failed" as const },
  { key: "pending", labelKey: "batch.pending", countKey: "pending" as const },
];

export function BatchTaskFilterBar({ current, onChange, summary }: BatchTaskFilterBarProps) {
  const { t } = useI18n();

  return (
    <div className="flex items-center gap-1 px-3 py-2 border-b border-border overflow-x-auto shrink-0">
      {filters.map((f) => {
        const count = summary ? summary[f.countKey] : 0;
        return (
          <button
            key={f.key}
            onClick={() => onChange(f.key)}
            className={cn(
              "flex items-center gap-1.5 px-2.5 py-1.5 rounded-md text-xs font-medium whitespace-nowrap transition-colors",
              current === f.key
                ? "bg-primary/10 text-primary"
                : "text-muted-foreground hover:bg-muted"
            )}
          >
            {t(f.labelKey)}
            <span className="tabular-nums opacity-70">({count})</span>
          </button>
        );
      })}
    </div>
  );
}