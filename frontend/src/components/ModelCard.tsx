import { Cpu, Download, RotateCcw, Loader2 } from "lucide-react";
import type { ModelInfo, DownloadProgress } from "../types";
import { useI18n } from "../i18n";
import { cn } from "../lib/utils";
import { Button } from "./ui/button";
import { Badge } from "./ui/badge";
import { Progress } from "./ui/progress";
import { useModelStore } from "../store/useModelStore";

interface ModelCardProps {
  model: ModelInfo;
}

export function ModelCard({ model }: ModelCardProps) {
  const { t } = useI18n();
  const { downloadModel, cancelDownload, downloading, downloadProgress } = useModelStore();

  const statusLabel = model.status === "downloaded"
    ? t("model.downloaded")
    : model.status === "downloading"
      ? t("model.downloading")
      : t("model.notDownloaded");

  const statusColor = model.status === "downloaded"
    ? "bg-success/10 text-success border-success/30"
    : model.status === "downloading"
      ? "bg-primary/10 text-primary border-primary/30"
      : "bg-muted text-muted-foreground border-border";

  const sizeGb = (model.sizeBytes / 1_000_000_000).toFixed(1);
  const progress = downloadProgress[model.name];

  return (
    <div className="rounded-lg border border-border p-4">
      <div className="flex items-start justify-between gap-3">
        <div className="flex items-center gap-3 min-w-0">
          <Cpu size={20} className="text-muted-foreground shrink-0" />
          <div className="min-w-0">
            <div className="flex items-center gap-2">
              <span className="text-sm font-medium truncate">{model.displayName}</span>
              <Badge variant="outline" className={cn("text-[10px]", statusColor)}>
                {statusLabel}
              </Badge>
            </div>
            <p className="text-xs text-muted-foreground mt-0.5">
              {model.name} · {sizeGb} {t("model.gb")}
            </p>
          </div>
        </div>

        <div className="flex items-center gap-1 shrink-0">
          {model.status === "downloaded" ? (
            <Button
              variant="outline"
              size="sm"
              className="h-7 text-xs"
              onClick={() => downloadModel(model.name)}
              disabled={downloading}
            >
              <RotateCcw size={12} className="mr-1" />
              {t("model.checkUpdate")}
            </Button>
          ) : model.status === "downloading" || (progress && progress.progress < 1.0) ? (
            <Button
              variant="outline"
              size="sm"
              className="h-7 text-xs"
              onClick={cancelDownload}
            >
              <Loader2 size={12} className="animate-spin mr-1" />
              {t("model.cancel")}
            </Button>
          ) : (
            <Button
              variant="default"
              size="sm"
              className="h-7 text-xs"
              onClick={() => downloadModel(model.name)}
              disabled={downloading}
            >
              <Download size={12} className="mr-1" />
              {t("model.download")}
            </Button>
          )}
        </div>
      </div>

      {progress && progress.progress > 0 && progress.progress < 1.0 && (
        <div className="mt-3">
          <Progress
            value={Math.round(progress.progress * 100)}
            indicatorClassName="bg-primary"
            className="h-2"
          />
          <div className="flex items-center justify-between mt-1">
            <span className="text-[10px] text-muted-foreground">
              {Math.round(progress.progress * 100)}%
            </span>
            {progress.speed && (
              <span className="text-[10px] text-muted-foreground">{progress.speed}</span>
            )}
          </div>
        </div>
      )}
    </div>
  );
}