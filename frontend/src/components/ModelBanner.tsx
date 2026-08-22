import { AlertTriangle, Download, Loader2, RotateCw } from "lucide-react";
import { useI18n } from "../i18n";
import { Button } from "./ui/button";
import { Progress } from "./ui/progress";

interface ModelBannerProps {
  preparing: boolean;
  prepareError: string | null;
  downloading: boolean;
  progress: number;
  onRetry: () => void;
  onCancelDownload?: () => void;
}

export function ModelBanner({
  preparing,
  prepareError,
  downloading,
  progress,
  onRetry,
  onCancelDownload,
}: ModelBannerProps) {
  const { t } = useI18n();

  return (
    <div className="border-b border-amber-300/40 bg-amber-500/10 px-3 py-1.5 flex items-center gap-2 text-xs">
      <AlertTriangle size={13} className="text-amber-500 shrink-0" />

      {prepareError ? (
        <>
          <span className="truncate text-amber-700 dark:text-amber-300">
            {t("banner.error")}
            <span className="opacity-70 ml-1">{prepareError}</span>
          </span>
          <div className="ml-auto flex items-center gap-1.5 shrink-0">
            <Button size="sm" className="h-6 px-2 text-[11px]" onClick={onRetry}>
              <RotateCw size={11} className="mr-1" />
              {t("banner.retry")}
            </Button>
          </div>
        </>
      ) : downloading ? (
        <>
          <span className="truncate text-amber-700 dark:text-amber-300">
            {t("banner.downloading")} {Math.round(progress * 100)}%
          </span>
          <div className="ml-auto flex items-center gap-2 shrink-0">
            <Progress
              value={Math.round(progress * 100)}
              indicatorClassName="bg-amber-500"
              className="h-1 w-28"
            />
            {onCancelDownload && (
              <button
                onClick={onCancelDownload}
                className="text-[11px] underline text-amber-700 dark:text-amber-300 hover:opacity-70 whitespace-nowrap"
              >
                {t("banner.cancel")}
              </button>
            )}
          </div>
        </>
      ) : preparing ? (
        <>
          <span className="truncate text-amber-700 dark:text-amber-300">
            {t("banner.preparing")}
          </span>
          <Loader2 size={13} className="ml-auto animate-spin shrink-0 text-amber-500" />
        </>
      ) : (
        <>
          <span className="truncate text-amber-700 dark:text-amber-300">
            {t("banner.text")}
          </span>
          <div className="ml-auto flex items-center gap-1.5 shrink-0">
            <Button size="sm" className="h-6 px-2 text-[11px]" onClick={onRetry}>
              <Download size={11} className="mr-1" />
              {t("banner.prepare")}
            </Button>
          </div>
        </>
      )}
    </div>
  );
}
