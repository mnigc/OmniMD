import { useState } from "react";
import {
  Copy,
  FileDown,
  FolderOpen,
  LayoutTemplate,
  Maximize2,
  Minimize2,
  RefreshCw,
  Code,
  Eye,
} from "lucide-react";
import { MarkdownPreview } from "../components/MarkdownPreview";
import { useTaskStore } from "../store/useTaskStore";

export function ConvertPage() {
  const { currentTask, currentResult } = useTaskStore();
  const [viewMode, setViewMode] = useState<"edit" | "preview" | "split">(
    "split"
  );
  const [markdown, setMarkdown] = useState("");
  const [outputPath, setOutputPath] = useState("");

  if (!currentTask || !currentResult) {
    return (
      <div className="h-full flex items-center justify-center">
        <div className="text-center">
          <FileDown className="mx-auto mb-4 text-muted-foreground opacity-50" size={48} />
          <p className="text-lg font-medium text-slate-900">
            No file to preview
          </p>
          <p className="text-sm text-muted-foreground mt-1">
            Drop a file on the Home page to start converting
          </p>
        </div>
      </div>
    );
  }

  return (
    <div className="h-full flex flex-col">
      <div className="border-b border-border px-4 py-2 flex items-center gap-3 shrink-0">
        <div className="flex items-center gap-2 px-3 py-1.5 bg-slate-50 rounded-md border border-border">
          <FileDown size={14} className="text-muted-foreground" />
          <span className="text-sm font-medium">{currentTask.sourcePath.split("/").pop()}</span>
        </div>

        <div className="flex items-center gap-1 ml-auto">
          <button
            onClick={() =>
              setViewMode(viewMode === "split" ? "edit" : "split")
            }
            className={`p-1.5 rounded transition-colors ${
              viewMode === "edit"
                ? "bg-violet-100 text-violet-700"
                : "hover:bg-slate-100 text-muted-foreground"
            }`}
            title="Edit only"
          >
            <Code size={16} />
          </button>
          <button
            onClick={() =>
              setViewMode(viewMode === "split" ? "preview" : "split")
            }
            className={`p-1.5 rounded transition-colors ${
              viewMode === "preview"
                ? "bg-violet-100 text-violet-700"
                : "hover:bg-slate-100 text-muted-foreground"
            }`}
            title="Preview only"
          >
            <Eye size={16} />
          </button>
          <button
            onClick={() => setViewMode("split")}
            className={`p-1.5 rounded transition-colors ${
              viewMode === "split"
                ? "bg-violet-100 text-violet-700"
                : "hover:bg-slate-100 text-muted-foreground"
            }`}
            title="Split view"
          >
            <LayoutTemplate size={16} />
          </button>
        </div>
      </div>

      <div className="flex-1 flex overflow-hidden">
        {(viewMode === "edit" || viewMode === "split") && (
          <div className="flex-1 flex flex-col border-r border-border">
            <div className="border-b border-border px-3 py-1.5 flex items-center gap-2 bg-slate-50">
              <span className="text-xs font-medium text-muted-foreground">
                Markdown
              </span>
            </div>
            <textarea
              className="flex-1 p-4 text-sm font-mono resize-none focus:outline-none"
              value={markdown}
              onChange={(e) => setMarkdown(e.target.value)}
              placeholder="Markdown content will appear here..."
              spellCheck={false}
            />
          </div>
        )}

        {(viewMode === "preview" || viewMode === "split") && (
          <div className="flex-1 flex flex-col">
            <div className="border-b border-border px-3 py-1.5 flex items-center gap-2 bg-slate-50">
              <span className="text-xs font-medium text-muted-foreground">
                Preview
              </span>
              <span className="text-[10px] text-muted-foreground ml-auto">
                {currentResult.assetCount} assets
              </span>
            </div>
            <div className="flex-1 overflow-auto p-4 prose prose-sm max-w-none">
              <MarkdownPreview content={markdown} />
            </div>
          </div>
        )}
      </div>

      <div className="border-t border-border px-4 py-3 flex items-center gap-2 bg-slate-50 shrink-0">
        <button
          onClick={() => {
            if (navigator.clipboard) {
              navigator.clipboard.writeText(markdown);
            }
          }}
          className="flex items-center gap-1.5 px-3 py-1.5 text-sm border border-slate-200 rounded-md hover:bg-slate-100 transition-colors"
        >
          <Copy size={14} />
          Copy Markdown
        </button>
        <button
          className="flex items-center gap-1.5 px-3 py-1.5 text-sm border border-slate-200 rounded-md hover:bg-slate-100 transition-colors"
        >
          <FileDown size={14} />
          Save .md
        </button>
        <button
          className="flex items-center gap-1.5 px-3 py-1.5 text-sm border border-slate-200 rounded-md hover:bg-slate-100 transition-colors"
        >
          <FolderOpen size={14} />
          Open Folder
        </button>
        <button
          className="flex items-center gap-1.5 px-3 py-1.5 text-sm border border-slate-200 rounded-md hover:bg-slate-100 transition-colors"
        >
          <RefreshCw size={14} />
          Re-convert
        </button>
        <button
          className="flex items-center gap-1.5 px-3 py-1.5 text-sm border border-slate-200 rounded-md hover:bg-slate-100 transition-colors ml-auto opacity-50"
          disabled
        >
          <Maximize2 size={14} />
          AI Optimize (Phase 3)
        </button>
      </div>
    </div>
  );
}
