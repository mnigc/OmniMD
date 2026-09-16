import { useEffect } from "react";
import { AlertCircle, Download, Loader2, RotateCw, X } from "lucide-react";
import { useI18n } from "../i18n";
import { useUpdateStore } from "../store/useUpdateStore";
import { Button } from "./ui/button";
import { Progress } from "./ui/progress";

export function UpdateDialog() {
  const { t } = useI18n();
  const {
    status,
    version,
    currentVersion,
    notes,
    date,
    progress,
    error,
    dialogOpen,
    install,
    restart,
    closeDialog,
  } = useUpdateStore();

  useEffect(() => {
    if (!dialogOpen) return;
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") closeDialog();
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [dialogOpen, closeDialog]);

  if (!dialogOpen) return null;

  const busy = status === "downloading" || status === "installing";
  const canClose = !busy;

  return (
    <div
      className="fixed inset-0 z-[9998] flex items-center justify-center bg-black/40 backdrop-blur-sm p-6"
      onClick={closeDialog}
    >
      <div
        role="dialog"
        aria-modal="true"
        aria-label={t("update.title")}
        className="w-full max-w-md rounded-xl border border-border bg-background shadow-2xl flex flex-col animate-fade-in"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="flex items-start gap-3 p-5 pb-3">
          <div className="w-9 h-9 rounded-lg bg-primary/10 text-primary flex items-center justify-center shrink-0">
            <Download size={18} />
          </div>
          <div className="min-w-0 flex-1">
            <h2 className="text-sm font-semibold">{t("update.title")}</h2>
            <p className="text-xs text-muted-foreground mt-0.5">
              {currentVersion && version
                ? t("update.versionTransition", {
                    from: currentVersion,
                    to: version,
                  })
                : version
                  ? t("update.available", { version })
                  : ""}
            </p>
          </div>
          {canClose && (
            <button
              type="button"
              onClick={closeDialog}
              aria-label={t("common.close")}
              className="text-muted-foreground hover:text-foreground transition-colors"
            >
              <X size={16} />
            </button>
          )}
        </div>

        <div className="px-5 pb-4 flex flex-col gap-3">
          {notes && (
            <div className="rounded-lg bg-muted/50 border border-border p-3 max-h-40 overflow-y-auto">
              <p className="text-xs text-muted-foreground whitespace-pre-line break-words">
                {notes}
              </p>
            </div>
          )}
          {date && !notes && (
            <p className="text-xs text-muted-foreground">{date}</p>
          )}

          {status === "downloading" && (
            <div className="flex flex-col gap-1.5">
              <Progress value={progress ?? 0} />
              <span className="text-xs text-muted-foreground">
                {progress === null
                  ? t("update.downloading")
                  : t("update.downloadingPercent", { percent: progress })}
              </span>
            </div>
          )}

          {status === "installing" && (
            <div className="flex items-center gap-2 text-xs text-muted-foreground">
              <Loader2 size={14} className="animate-spin" />
              <span>{t("update.installing")}</span>
            </div>
          )}

          {status === "ready" && (
            <div className="flex items-center gap-2 text-xs text-muted-foreground">
              <RotateCw size={14} />
              <span>{t("update.ready")}</span>
            </div>
          )}

          {status === "error" && error && (
            <div className="flex items-start gap-2 text-xs text-destructive">
              <AlertCircle size={14} className="mt-0.5 shrink-0" />
              <span className="break-words">
                {t("update.installFailed")}: {error}
              </span>
            </div>
          )}
        </div>

        <div className="flex items-center justify-end gap-2 px-5 py-4 border-t border-border">
          {status === "ready" ? (
            <Button size="sm" onClick={() => restart()}>
              <RotateCw size={14} />
              {t("update.restart")}
            </Button>
          ) : (
            <>
              <Button
                size="sm"
                variant="outline"
                onClick={closeDialog}
                disabled={!canClose}
              >
                {t("update.later")}
              </Button>
              <Button size="sm" onClick={() => install()} disabled={busy}>
                {busy ? (
                  <Loader2 size={14} className="animate-spin" />
                ) : (
                  <Download size={14} />
                )}
                {status === "error" ? t("common.retry") : t("update.download")}
              </Button>
            </>
          )}
        </div>
      </div>
    </div>
  );
}
