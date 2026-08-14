import { useCallback, useEffect, useRef, useState } from "react";
import { FileUp, FolderOpen, Upload, X } from "lucide-react";

interface DropZoneProps {
  onFiles: (paths: string[]) => void;
  disabled: boolean;
  formats: string[];
}

function isSupportedFile(file: File, formats: string[]): boolean {
  if (formats.length === 0) return true;
  const ext = file.name.split(".").pop()?.toLowerCase();
  return ext ? formats.includes(ext) : false;
}

function walkDirectory(
  entry: FileSystemEntry,
  formats: string[]
): Promise<string[]> {
  return new Promise((resolve) => {
    const results: string[] = [];

    if (entry.isFile) {
      const fileEntry = entry as FileSystemFileEntry;
      const name = fileEntry.name;
      const ext = name.split(".").pop()?.toLowerCase();
      if (!ext || !formats.includes(ext)) {
        resolve(results);
        return;
      }
      results.push(name);
      resolve(results);
      return;
    }

    if (entry.isDirectory) {
      const dirReader = (entry as FileSystemDirectoryEntry)
        .createReader();
      const entries: FileSystemEntry[] = [];

      const readBatch = () => {
        dirReader.readEntries((batch) => {
          if (batch.length === 0) {
            const promises = entries.map((e) =>
              walkDirectory(e, formats)
            );
            Promise.all(promises).then((allResults) => {
              allResults.flat().forEach((r) => results.push(r));
              resolve(results);
            });
            return;
          }
          entries.push(...batch);
          readBatch();
        });
      };

      readBatch();
      return;
    }

    resolve(results);
  });
}

export function DropZone({ onFiles, disabled, formats }: DropZoneProps) {
  const [isDragging, setIsDragging] = useState(false);
  const [dragCount, setDragCount] = useState(0);
  const [filePaths, setFilePaths] = useState<string[]>([]);
  const dropRef = useRef<HTMLDivElement>(null);

  const processFiles = useCallback(
    (entries: FileSystemEntry[]) => {
      const promises = entries.map((e) => walkDirectory(e, formats));
      Promise.all(promises).then((results) => {
        const allPaths = results.flat();
        setFilePaths(allPaths);
        if (allPaths.length > 0) {
          onFiles(allPaths);
        }
      });
    },
    [formats, onFiles]
  );

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

      const items = Array.from(e.dataTransfer.items || []);
      const entries: FileSystemEntry[] = [];

      for (const item of items) {
        const entry = (item as any).webkitGetAsEntry?.();
        if (entry) {
          entries.push(entry);
        }
      }

      if (entries.length > 0) {
        processFiles(entries);
        return;
      }

      const files = Array.from(e.dataTransfer.files || []);
      const validPaths = files
        .filter((f) => isSupportedFile(f, formats))
        .map((f) => f.name);

      if (validPaths.length > 0) {
        setFilePaths(validPaths);
        onFiles(validPaths);
      }
    },
    [processFiles, formats, onFiles]
  );

  return (
    <div
      ref={dropRef}
      onDragEnter={handleDragEnter}
      onDragLeave={handleDragLeave}
      onDragOver={handleDragOver}
      onDrop={handleDrop}
      className={`relative flex flex-col items-center justify-center p-8 rounded-xl border-2 border-dashed transition-all duration-200 ${
        disabled
          ? "border-slate-200 bg-slate-50 opacity-60 cursor-not-allowed"
          : isDragging
            ? "border-violet-500 bg-violet-50/50 scale-[1.01] shadow-lg shadow-violet-100"
            : "border-slate-300 bg-slate-50/50 hover:border-slate-400 hover:bg-slate-100/50"
      }`}
    >
      <div
        className={`mb-4 p-4 rounded-full transition-colors ${
          isDragging ? "bg-violet-100 text-violet-600" : "bg-slate-200 text-slate-500"
        }`}
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
            ? "Drop files to convert"
            : "Drop files or folders here"}
      </p>
      <p className="text-sm text-muted-foreground text-center">
        Supported: DOCX, PDF, PPTX, XLSX, EPUB, CSV, TXT, HTML, RTF, ODT...
      </p>

      {filePaths.length > 0 && (
        <div className="mt-4 flex items-center gap-2 px-3 py-1.5 bg-green-50 border border-green-200 rounded-md">
          <CheckmarkIcon className="text-green-600" size={14} />
          <span className="text-xs text-green-700">
            {filePaths.length} file{filePaths.length > 1 ? "s" : ""} detected
          </span>
        </div>
      )}
    </div>
  );
}

function CheckmarkIcon({ size }: { size: number }) {
  return (
    <svg
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
