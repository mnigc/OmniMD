import { useCallback, useEffect, useRef, useState } from "react";
import { FileUp, FolderOpen, Upload } from "lucide-react";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import type { UnlistenFn } from "@tauri-apps/api/event";
import { pickFiles, pickDir } from "../api/dialogs";
import { useI18n } from "../i18n";
import { cn } from "../lib/utils";
import { Button } from "./ui/button";

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

  useEffect(() => {
    let unlisten: UnlistenFn | undefined;
    tauriDndReady.current = false;
    (async () => {
      try {
        const appWindow = getCurrentWebviewWindow();
        unlisten = await appWindow.onDragDropEvent((event) => {
          const type = event.payload.type;
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
              onFiles(paths);
            }
          }
        });
        tauriDndReady.current = true;
      } catch {
        /* non-Tauri, fall back to DOM drag/drop */
      }
    })();
    return () => {
      unlisten?.();
      tauriDndReady.current = false;
    };
  }, [onFiles]);

  const openFilePicker = useCallback(async () => {
    const paths = await pickFiles(formats);
    if (paths.length > 0) {
      onFiles(paths);
    }
  }, [formats, onFiles]);

  const openFolderPicker = useCallback(async () => {
    if (!onFolder) return;
    const dir = await pickDir();
    if (dir) onFolder(dir);
  }, [onFolder]);

  const handleDragOver = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    e.stopPropagation();
  }, []);

  const handleDragEnter = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    e.stopPropagation();
  }, []);

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

      if (tauriDndReady.current) return;

      const dt = e.dataTransfer;
      if (!dt) return;
      const files = dt.files;
      if (files && files.length > 0) {
        const paths: string[] = [];
        for (let i = 0; i < files.length; i++) {
          const f = files[i] as any;
          if (f.path) paths.push(f.path);
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
        "flex items-center gap-3 px-4 py-3 rounded-xl border-2 border-dashed transition-all duration-200",
        className,
        isDragging
          ? "border-primary bg-primary/5 shadow-lg shadow-primary/10"
          : "border-border bg-muted/30 hover:border-primary/50 hover:bg-muted/50"
      )}
    >
      <div
        className={cn(
          "p-2.5 rounded-full shrink-0 transition-colors",
          isDragging
            ? "bg-primary/10 text-primary"
            : "bg-muted text-muted-foreground"
        )}
      >
        {isDragging ? (
          isFolderDrag ? (
            <FolderOpen size={20} />
          ) : (
            <Upload size={20} className="animate-bounce" />
          )
        ) : (
          <FileUp size={20} />
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
      </div>

      <div className="flex items-center gap-2 shrink-0">
        <Button
          size="sm"
          variant="outline"
          onClick={(e) => {
            e.stopPropagation();
            openFilePicker();
          }}
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
          >
            <FolderOpen size={14} />
            {t("home.chooseFolder")}
          </Button>
        )}
      </div>
    </div>
  );
}