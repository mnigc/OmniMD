import { useCallback, useEffect, useRef, useState } from "react";
import { FileUp, FolderOpen, Upload } from "lucide-react";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import type { UnlistenFn } from "@tauri-apps/api/event";
import { pickFiles, pickDir } from "../api/dialogs";
import { useI18n } from "../i18n";
import { cn } from "../lib/utils";
import { Button } from "./ui/button";
import { useModelStore } from "../store/useModelStore";
import { showToast } from "../lib/toast";

interface DropZoneProps {
  onFiles: (paths: string[]) => void;
  onFolder?: (path: string) => void;
  formats: string[];
  className?: string;
}

export function DropZone({
  onFiles,
  onFolder,
  formats,
  className,
}: DropZoneProps) {
  const { t } = useI18n();
  const [isDragging, setIsDragging] = useState(false);
  const [isFolderDrag, setIsFolderDrag] = useState(false);
  const tauriDndReady = useRef(false);
const dropHandledNatively = useRef(false);
  // Keep the latest `onFiles` in a ref so the native drag-drop listener can be
  // attached exactly once for the component's lifetime. Attaching inside an
  // effect keyed on `onFiles` leaks listeners (the async `unlisten` is still
  // undefined when the cleanup runs), which caused a single drop to enqueue the
  // same file multiple times.
  const onFilesRef = useRef(onFiles);
  onFilesRef.current = onFiles;
  const { modelReady } = useModelStore();
  const disabled = !modelReady;

  useEffect(() => {
    let unlisten: UnlistenFn | undefined;
    let cleanedUp = false;
    tauriDndReady.current = false;
    (async () => {
      try {
        const appWindow = getCurrentWebviewWindow();
        unlisten = await appWindow.onDragDropEvent((event) => {
          const type = event.payload.type;
          const isDisabled = () => {
            const s = useModelStore.getState();
            return !s.modelReady;
          };
          if (type === "enter") {
            const paths = (event.payload as { paths?: string[] }).paths ?? [];
            const single = paths.length === 1 ? paths[0] : undefined;
            setIsFolderDrag(
              single !== undefined &&
                !(single.split(/[\\/]/).pop() ?? "").includes(".")
            );
            setIsDragging(true);
          } else if (type === "over") {
            setIsDragging(true);
          } else if (type === "leave") {
            setIsDragging(false);
            setIsFolderDrag(false);
          } else if (type === "drop") {
            setIsDragging(false);
            setIsFolderDrag(false);
            const paths = (event.payload as { paths?: string[] }).paths ?? [];
            if (paths.length) {
              dropHandledNatively.current = true;
              try {
                onFilesRef.current(paths);
              } catch (err) {
                console.error("Native drop handler failed:", err);
              }
            }
          }
        });
        tauriDndReady.current = true;
        // The effect may have been cleaned up while we awaited the listener.
        if (cleanedUp) {
          unlisten();
        }
      } catch {
        /* non-Tauri, fall back to DOM drag/drop */
      }
    })();
    return () => {
      cleanedUp = true;
      unlisten?.();
      tauriDndReady.current = false;
    };
  }, []);

  const openFilePicker = useCallback(async () => {
    try {
      const paths = await pickFiles(formats);
      if (paths.length > 0) {
        onFiles(paths);
      }
    } catch (err) {
      console.error("File picker failed:", err);
      showToast(t("toast.filePickFailed"), 3000);
    }
  }, [formats, onFiles, t]);

  const openFolderPicker = useCallback(async () => {
    if (!onFolder) return;
    try {
      const dir = await pickDir();
      if (dir) onFolder(dir);
    } catch (err) {
      console.error("Folder picker failed:", err);
      showToast(t("toast.folderPickFailed"), 3000);
    }
  }, [onFolder, t]);

  const handleDragOver = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    e.stopPropagation();
  }, []);

  const handleDragEnter = useCallback(
    (e: React.DragEvent) => {
      e.preventDefault();
      e.stopPropagation();
      const s = useModelStore.getState();
      if (!s.modelReady) return;
      setIsDragging(true);
    },
    []
  );

  const handleDragLeave = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    e.stopPropagation();
    setIsDragging(false);
    setIsFolderDrag(false);
  }, []);

  const handleDrop = useCallback(
    async (e: React.DragEvent) => {
      e.preventDefault();
      e.stopPropagation();
      setIsDragging(false);
      setIsFolderDrag(false);

      if (dropHandledNatively.current) {
        dropHandledNatively.current = false;
        return;
      }

      const dt = e.dataTransfer;
      if (!dt) return;
      const files = dt.files;
      if (files && files.length > 0) {
        const paths: string[] = [];
        for (let i = 0; i < files.length; i++) {
          const f = files[i] as any;
          if (f.path) {
            paths.push(f.path);
          } else if (f.webkitRelativePath) {
            // Fallback: if no path but has relative path, it's a folder drop via DOM
          }
        }
        if (paths.length > 0) {
          onFiles(paths);
          return;
        }
      }
      if (dt.items && dt.items.length > 0) {
        const item = dt.items[0];
        if (item.kind === "file") {
          const entry = item.webkitGetAsEntry?.();
          if (entry && entry.isDirectory && onFolder) {
            openFolderPicker();
            return;
          }
        }
      }
      openFilePicker();
    },
    [onFiles, onFolder, openFilePicker, openFolderPicker]
  );

  return (
    <div
      onDragEnter={handleDragEnter}
      onDragLeave={handleDragLeave}
      onDragOver={handleDragOver}
      onDrop={handleDrop}
      className={cn(
        "group relative flex items-center gap-4 px-5 py-4 rounded-xl border-2 border-dashed",
        "transition-all duration-300 ease-out",
        className,
        isDragging
          ? "border-primary bg-primary/8 shadow-xl shadow-primary/15 scale-[1.01]"
          : disabled
            ? "border-border bg-muted/20 opacity-50 cursor-not-allowed"
            : "border-border/70 bg-muted/20 hover:border-primary/50 hover:bg-muted/40 hover:shadow-md hover:shadow-primary/5"
      )}
    >
      <div
        className={cn(
          "p-3 rounded-xl shrink-0 transition-all duration-300",
          isDragging
            ? "bg-primary/15 text-primary scale-110"
            : "bg-muted/60 text-muted-foreground group-hover:text-primary group-hover:bg-primary/10"
        )}
      >
        {isDragging ? (
          isFolderDrag ? (
            <FolderOpen size={24} />
          ) : (
            <Upload size={24} className="animate-bounce" />
          )
        ) : (
          <FileUp size={24} />
        )}
      </div>

      <div className="flex-1 min-w-0">
        <p className="text-sm font-medium truncate">
          {isDragging
            ? isFolderDrag
              ? t("dropzone.folderDetected")
              : t("dropzone.releaseToConvert")
            : t("dropzone.dropFilesOrFolder")}
        </p>
        <p
          className={cn(
            "text-xs mt-1",
            disabled ? "text-destructive" : "text-muted-foreground"
          )}
        >
          {disabled
            ? t("dropzone.disabledHint")
            : t("dropzone.localLimits")}
        </p>
      </div>

      <div className="flex items-center gap-2 shrink-0">
        <Button
          size="sm"
          variant="outline"
          onClick={(e) => {
            e.stopPropagation();
            openFilePicker();
          }}
          className="transition-all duration-200"
        >
          <FileUp size={14} />
          {t("home.addFiles")}
        </Button>
        {onFolder && (
          <Button
            size="sm"
            variant="outline"
            onClick={(e) => {
              e.stopPropagation();
              openFolderPicker();
            }}
            className="transition-all duration-200"
          >
            <FolderOpen size={14} />
            {t("home.chooseFolder")}
          </Button>
        )}
      </div>
    </div>
  );
}