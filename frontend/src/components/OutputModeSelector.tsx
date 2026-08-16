import { useI18n } from "../i18n";
import { useSettingsStore } from "../store/useSettingsStore";
import type { OutputMode } from "../types";
import { cn } from "../lib/utils";

interface OutputModeSelectorProps {
  disabled?: boolean;
  className?: string;
}

const MODES: OutputMode[] = ["standard", "aiReady", "obsidian"];

export function OutputModeSelector({
  disabled,
  className,
}: OutputModeSelectorProps) {
  const { t } = useI18n();
  const { outputMode, setOutputMode } = useSettingsStore();

  const labelKey: Record<OutputMode, string> = {
    standard: "outputMode.standard",
    aiReady: "outputMode.aiReady",
    obsidian: "outputMode.obsidian",
  };

  const descKey: Record<OutputMode, string> = {
    standard: "outputMode.standardDesc",
    aiReady: "outputMode.aiReadyDesc",
    obsidian: "outputMode.obsidianDesc",
  };

  return (
    <div className={cn("flex flex-col gap-1.5", className)}>
      <label className="text-xs text-muted-foreground">
        {t("home.outputMode")}
      </label>
      <div className="grid grid-cols-3 gap-2">
        {MODES.map((mode) => (
          <button
            key={mode}
            type="button"
            disabled={disabled}
            onClick={() => setOutputMode(mode)}
            className={cn(
              "flex flex-col items-center gap-0.5 px-3 py-2 rounded-lg border text-center transition-all",
              outputMode === mode
                ? "border-primary bg-primary/5 text-primary"
                : "border-border bg-muted/30 hover:border-primary/40 hover:bg-muted/50",
              disabled && "opacity-50 cursor-not-allowed"
            )}
          >
            <span className="text-xs font-medium">
              {t(labelKey[mode])}
            </span>
            <span className="text-[10px] text-muted-foreground leading-tight">
              {t(descKey[mode])}
            </span>
          </button>
        ))}
      </div>
    </div>
  );
}
