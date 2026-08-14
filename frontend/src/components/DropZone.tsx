import { useCallback, useState } from "react";
import { FileUp, Upload, X } from "lucide-react";
import { pickFiles } from "../api/dialogs";
import { cn } from "../lib/utils";

interface DropZoneProps {
  onFiles: (paths: string[]) => void;
  disabled: boolean;
  formats: string[];
}

export function DropZone({ onFiles, disabled, formats }: DropZoneProps) {
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
      // Browser sandboxing prevents access to full file paths from dropped
      // files, so open the native file dialog to resolve real paths.
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
        "relative flex flex-col items-center justify-center p-8 rounded-xl border-2 border-dashed transition-all duration-200 cursor-pointer",
        disabled
          ? "border-slate-200 bg-slate-50 opacity-60 cursor-not-allowed"
          : isDragging
            ? "border-violet-500 bg-violet-50/50 scale-[1.01] shadow-lg shadow-violet-100"
            : "border-slate-300 bg-slate-50/50 hover:border-slate-400 hover:bg-slate-100/50"
      )}
    >
      <div
        className={cn(
          "mb-4 p-4 rounded-full transition-colors",
          isDragging
            ? "bg-violet-100 text-violet-600"
            : "bg-slate-200 text-slate-500"
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

      <p className="text-base font-medium text-slate-900 mb-1">
        {disabled
          ? "Drop disabled"
          : isDragging
            ? "Release to open file picker"
            : "Drop files here or click to browse"}
      </p>
      <p className="text-sm text-muted-foreground text-center">
        Supported: DOCX, PDF, PPTX, XLSX, EPUB, CSV, TXT, HTML, RTF, ODT...
      </p>

      {selectedCount > 0 && (
        <div className="mt-4 flex items-center gap-2 px-3 py-1.5 bg-green-50 border border-green-200 rounded-md">
          <CheckmarkIcon className="text-green-600" size={14} />
          <span className="text-xs text-green-700">
            {selectedCount} file{selectedCount > 1 ? "s" : ""} selected
          </span>
        </div>
      )}
    </div>
  );
}

function CheckmarkIcon({
  size,
  className,
}: {
  size: number;
  className?: string;
}) {
  return (
    <svg
      className={className}
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="2"
      strokeLinecap="round"
      strokeLinejoin="round"
    >
      <polyline points="20 6 9 17 4 12" />
    </svg>
  );
}