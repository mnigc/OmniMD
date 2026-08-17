import { FolderOpen, Trash2, Loader2 } from "lucide-react";
import { useI18n } from "../i18n";
import { useModelStore } from "../store/useModelStore";
import { Button } from "./ui/button";

export function ModelCacheSection() {
  const { t } = useI18n();
  const { cacheInfo, clearCache } = useModelStore();

  if (!cacheInfo) return null;

  const sizeGb = (cacheInfo.totalSizeBytes / 1_000_000_000).toFixed(2);

  return (
    <div className="space-y-3">
      <div className="flex items-center justify-between py-2">
        <span className="text-sm text-muted-foreground">{t("model.cacheSize")}</span>
        <span className="text-sm font-medium tabular-nums">{sizeGb} {t("model.gb")}</span>
      </div>
      <div className="flex items-center justify-between py-2">
        <span className="text-sm text-muted-foreground">{t("model.cachePath")}</span>
        <span className="text-xs text-muted-foreground truncate max-w-[200px] ml-2" title={cacheInfo.path}>
          {cacheInfo.path}
        </span>
      </div>
      <div className="flex items-center gap-2 pt-2">
        <Button
          variant="outline"
          size="sm"
          className="text-xs"
          onClick={async () => {
            try {
              const { openFolder } = await import("../api/tauriApi");
              await openFolder(cacheInfo.path);
            } catch {
              // ignore
            }
          }}
        >
          <FolderOpen size={12} className="mr-1" />
          {t("model.openDir")}
        </Button>
        <Button
          variant="outline"
          size="sm"
          className="text-xs text-destructive"
          onClick={async () => {
            if (window.confirm(t("model.clearCacheConfirm"))) {
              await clearCache();
            }
          }}
        >
          <Trash2 size={12} className="mr-1" />
          {t("model.clearCache")}
        </Button>
      </div>
    </div>
  );
}