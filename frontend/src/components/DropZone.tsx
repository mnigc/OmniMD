import { useCallback, useState } from "react";
import { Check, FileUp, Upload, X } from "lucide-react";
import { pickFiles } from "../api/dialogs";
import { useI18n } from "../i18n";
import { cn } from "../lib/utils";
import { Badge } from "./ui/badge";

interface DropZoneProps {
  onFiles: (paths: string[]) => void;
  disabled: boolean;
  formats: string[];
  className?: string;
}

export function DropZone({
  onFiles,
  disabled,
  formats,
  className,
}: DropZoneProps) {
  const { t } = useI18n();
  const [isDragging, setIsDragging] = useState(false);
  const [dragCount, setDragCount] = useState(0);
  const [selectedCount, setSelectedCount] = useState(0);

  const openFilePicker = useCallback(async () => {
    if (disabled) return;
    const paths = await pickFiles(formats);
    if (paths.length > 0) {
      setSelectedCount(paths.length);
      onFiles(paths);
    }
  }, [disabled, formats, onFiles]);

  const handleDragEnter = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    e.stopPropagation();
    setDragCount((c) => c + 1);
    setIsDragging(true);
  }, []);

  const handleDragLeave = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    e.stopPropagation();
    setDragCount((c) => c - 1);
    if (dragCount <= 1) {
      setIsDragging(false);
    }
  }, [dragCount]);

  const handleDragOver = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    e.stopPropagation();
  }, []);

  const handleDrop = useCallback(
    (e: React.DragEvent) => {
      e.preventDefault();
      e.stopPropagation();
      setIsDragging(false);
      setDragCount(0);
      openFilePicker();
    },
    [openFilePicker]
  );

  return (
    <div
      onDragEnter={handleDragEnter}
      onDragLeave={handleDragLeave}
      onDragOver={handleDragOver}
      onDrop={handleDrop}
      onClick={openFilePicker}
      role="button"
      aria-disabled={disabled}
      className={cn(
        "relative flex flex-col items-center justify-center p-8 rounded-xl border-2 border-dashed transition-all duration-200 cursor-pointer focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 focus-visible:ring-offset-background",
        className,
        disabled
          ? "border-input bg-muted/50 opacity-60 cursor-not-allowed"
          : isDragging
            ? "border-primary bg-primary/5 scale-[1.01] shadow-lg shadow-primary/10"
            : "border-border bg-muted/30 hover:border-primary/50 hover:bg-muted/50"
      )}
    >
      <div
        className={cn(
          "mb-4 p-4 rounded-full transition-colors",
          isDragging
            ? "bg-primary/10 text-primary"
            : "bg-muted text-muted-foreground"
        )}
      >
        {disabled ? (
          <X size={28} />
        ) : isDragging ? (
          <Upload size={28} className="animate-bounce" />
        ) : (
          <FileUp size={28} />
        )}
      </div>

      <p className="text-base font-medium mb-1">
        {disabled
          ? t("dropzone.dropDisabled")
          : isDragging
            ? t("dropzone.releaseToOpen")
            : t("dropzone.dropFiles")}
      </p>
      <p className="text-sm text-muted-foreground text-center">
        {t("dropzone.supported")}
      </p>

      {selectedCount > 0 && (
        <Badge variant="success" className="mt-4 gap-1">
          <Check size={12} />
          {selectedCount} {t("dropzone.files")} {t("dropzone.selected")}
        </Badge>
      )}
    </div>
  );
}