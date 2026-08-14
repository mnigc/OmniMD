import { useEffect, useState } from "react";
import {
  Copy,
  FileDown,
  FolderOpen,
  LayoutTemplate,
  Maximize2,
  Code,
  Eye,
  RefreshCw,
  ArrowLeft,
} from "lucide-react";
import { MarkdownPreview } from "../components/MarkdownPreview";
import { SegmentedControl } from "../components/SegmentedControl";
import { useTaskStore } from "../store/useTaskStore";
import { useI18n } from "../i18n";
import { Button } from "../components/ui/button";
import { Separator } from "../components/ui/separator";

type ViewMode = "edit" | "preview" | "split";

interface ConvertPageProps {
  onNavigate?: (page: "home") => void;
}

export function ConvertPage({ onNavigate }: ConvertPageProps) {
  const { t } = useI18n();
  const { currentTask, currentResult } = useTaskStore();
  const [viewMode, setViewMode] = useState<ViewMode>("split");
  const [markdown, setMarkdown] = useState("");

  useEffect(() => {
    if (currentResult?.markdown) {
      setMarkdown(currentResult.markdown);
    }
  }, [currentResult]);

  const viewOptions: {
    value: ViewMode;
    icon: React.ReactNode;
    label: string;
  }[] = [
    { value: "edit", icon: <Code size={16} />, label: t("convert.editOnly") },
    {
      value: "preview",
      icon: <Eye size={16} />,
      label: t("convert.previewOnly"),
    },
    {
      value: "split",
      icon: <LayoutTemplate size={16} />,
      label: t("convert.splitView"),
    },
  ];

  if (!currentTask || !currentResult) {
    return (
      <div className="h-full flex items-center justify-center">
        <div className="text-center">
          <FileDown
            className="mx-auto mb-4 text-muted-foreground opacity-50"
            size={48}
          />
          <p className="text-lg font-medium">{t("convert.noFile")}</p>
          <p className="text-sm text-muted-foreground mt-1">
            {t("convert.noFileHint")}
          </p>
          {onNavigate && (
            <Button
              variant="outline"
              size="sm"
              className="mt-4"
              onClick={() => onNavigate("home")}
            >
              <ArrowLeft size={14} />
              {t("nav.home")}
            </Button>
          )}
        </div>
      </div>
    );
  }

  return (
    <div className="h-full flex flex-col">
      <div className="border-b border-border px-4 py-2 flex items-center gap-3 shrink-0">
        <div className="flex items-center gap-2 h-8 px-3 bg-muted rounded-md">
          <FileDown size={14} className="text-muted-foreground" />
          <span className="text-sm font-medium truncate max-w-72">
            {currentTask.sourcePath.split("/").pop()}
          </span>
        </div>

        <div className="ml-auto">
          <SegmentedControl
            options={viewOptions}
            value={viewMode}
            onChange={setViewMode}
            size="md"
          />
        </div>
      </div>

      <div className="flex-1 flex overflow-hidden">
        {(viewMode === "edit" || viewMode === "split") && (
          <div className="flex-1 flex flex-col min-w-0 border-r border-border">
            <div className="h-9 border-b border-border px-3 flex items-center gap-2 bg-muted/30">
              <span className="text-xs font-medium text-muted-foreground">
                {t("convert.markdown")}
              </span>
            </div>
            <textarea
              className="flex-1 p-4 text-sm font-mono resize-none focus:outline-none bg-transparent"
              value={markdown}
              onChange={(e) => setMarkdown(e.target.value)}
              placeholder={t("convert.markdownPlaceholder")}
              spellCheck={false}
            />
          </div>
        )}

        {(viewMode === "preview" || viewMode === "split") && (
          <div className="flex-1 flex flex-col min-w-0">
            <div className="h-9 border-b border-border px-3 flex items-center gap-2 bg-muted/30">
              <span className="text-xs font-medium text-muted-foreground">
                {t("convert.preview")}
              </span>
              <span className="text-xs text-muted-foreground ml-auto">
                {currentResult.assetCount} {t("convert.assets")}
              </span>
            </div>
            <div className="flex-1 overflow-auto p-4 prose prose-sm max-w-none">
              <MarkdownPreview content={markdown} />
            </div>
          </div>
        )}
      </div>

      <div className="border-t border-border bg-muted/30 px-4 py-3 flex items-center gap-2 shrink-0">
        <Button
          variant="outline"
          size="sm"
          onClick={() => {
            if (navigator.clipboard) {
              navigator.clipboard.writeText(markdown);
            }
          }}
        >
          <Copy size={14} />
          {t("convert.copyMarkdown")}
        </Button>
        <Button variant="outline" size="sm">
          <FileDown size={14} />
          {t("convert.saveMd")}
        </Button>
        <Button variant="outline" size="sm">
          <FolderOpen size={14} />
          {t("convert.openFolder")}
        </Button>
        <Button variant="outline" size="sm">
          <RefreshCw size={14} />
          {t("convert.reconvert")}
        </Button>

        <Separator orientation="vertical" className="h-6 mx-1" />

        <Button
          className="ml-auto opacity-50"
          variant="outline"
          size="sm"
          disabled
        >
          <Maximize2 size={14} />
          {t("convert.aiOptimize")}
        </Button>
      </div>
    </div>
  );
}