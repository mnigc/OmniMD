import { AlertTriangle, Download } from "lucide-react";
import { useI18n } from "../i18n";
import { Button } from "./ui/button";
import { Progress } from "./ui/progress";

interface ModelBannerProps {
  downloading: boolean;
  progress: number;
  onDownload: () => void;
  onCancelDownload?: () => void;
}

export function ModelBanner({
  downloading,
  progress,
  onDownload,
  onCancelDownload,
}: ModelBannerProps) {
  const { t } = useI18n();

  return (
    <div className="border-b border-amber-300/40 bg-amber-500/10 px-3 py-1.5 flex items-center gap-2 text-xs">
      <AlertTriangle size={13} className="text-amber-500 shrink-0" />
      <span className="truncate text-amber-700 dark:text-amber-300">
        {t("banner.text")}
      </span>

      {downloading ? (
        <div className="ml-auto flex items-center gap-2 shrink-0">
          <Progress
            value={Math.round(progress * 100)}
            indicatorClassName="bg-amber-500"
            className="h-1 w-28"
          />
          <span className="text-[11px] text-amber-700 dark:text-amber-300 tabular-nums whitespace-nowrap">
            {t("banner.downloading")} {Math.round(progress * 100)}%
          </span>
          {onCancelDownload && (
            <button
              onClick={onCancelDownload}
              className="text-[11px] underline text-amber-700 dark:text-amber-300 hover:opacity-70 whitespace-nowrap"
            >
              {t("banner.cancel")}
            </button>
          )}
        </div>
      ) : (
        <div className="ml-auto flex items-center gap-1.5 shrink-0">
          <Button size="sm" className="h-6 px-2 text-[11px]" onClick={onDownload}>
            <Download size={11} className="mr-1" />
            {t("banner.download")}
          </Button>
        </div>
      )}
    </div>
  );
}