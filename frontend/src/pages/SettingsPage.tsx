import { useCallback, useEffect, useState } from "react";
import { Monitor, Moon, Sun, ShieldCheck, FolderOpen, Sparkles, Cpu } from "lucide-react";
import { useI18n } from "../i18n";
import { type ThemeMode } from "../lib/theme";
import { useThemeMode } from "../hooks/useThemeMode";
import { PageHeader } from "../components/PageHeader";
import { OutputModeSelector } from "../components/OutputModeSelector";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "../components/ui/card";
import { Input } from "../components/ui/input";
import { Button } from "../components/ui/button";
import { cn } from "../lib/utils";
import { useSettingsStore } from "../store/useSettingsStore";
import { mineruStatus, startMineru, type MineruStatus } from "../api/tauriApi";
import { pickOutputDir } from "../api/dialogs";
import type { ParseQuality } from "../types";

const themeOptions: {
  value: ThemeMode;
  icon: React.ReactNode;
  labelKey: string;
}[] = [
  { value: "light", icon: <Sun size={16} />, labelKey: "theme.light" },
  { value: "dark", icon: <Moon size={16} />, labelKey: "theme.dark" },
  { value: "auto", icon: <Monitor size={16} />, labelKey: "theme.auto" },
];

function ToggleRow({
  label,
  checked,
  disabled,
  onChange,
  hint,
}: {
  label: string;
  checked: boolean;
  disabled?: boolean;
  onChange?: (v: boolean) => void;
  hint?: string;
}) {
  return (
    <div className="flex items-center justify-between gap-4 py-2.5">
      <div className="flex flex-col">
        <span className="text-sm text-muted-foreground">{label}</span>
        {hint && (
          <span className="text-xs text-muted-foreground/70 mt-0.5">{hint}</span>
        )}
      </div>
      <div className="flex items-center gap-2">
        <input
          type="checkbox"
          disabled={disabled}
          checked={checked}
          onChange={(e) => onChange?.(e.target.checked)}
          className={cn("h-4 w-4", disabled && "opacity-50")}
        />
      </div>
    </div>
  );
}

function InfoRow({
  label,
  children,
}: {
  label: string;
  children: React.ReactNode;
}) {
  return (
    <div className="flex items-center justify-between gap-4 py-2.5">
      <span className="text-sm text-muted-foreground">{label}</span>
      <div className="text-sm text-right">{children}</div>
    </div>
  );
}

export function SettingsPage() {
  const { t } = useI18n();
  const { theme, setMode } = useThemeMode();
  const {
    parseQuality,
    setParseQuality,
    defaultOutputDir,
    recursive,
    keepStructure,
    aiEnabled,
    aiReadyToc,
    aiReadyMeta,
    setDefaultOutputDir,
    setRecursive,
    setKeepStructure,
    setAiEnabled,
    setAiReadyToc,
    setAiReadyMeta,
  } = useSettingsStore();

  const [mineruInfo, setMineruInfo] = useState<MineruStatus | null>(null);
  const [mineruStarting, setMineruStarting] = useState(false);
  const [mineruError, setMineruError] = useState<string | null>(null);

  const refreshMineruStatus = useCallback(async () => {
    try {
      const info = await mineruStatus();
      setMineruInfo(info);
      setMineruError(null);
    } catch (e) {
      setMineruInfo(null);
      setMineruError(String(e));
    }
  }, []);

  useEffect(() => {
    refreshMineruStatus();
  }, [refreshMineruStatus]);

  const handleStartMineru = async () => {
    setMineruStarting(true);
    setMineruError(null);
    try {
      await startMineru();
      await refreshMineruStatus();
    } catch (e) {
      setMineruError(String(e));
    } finally {
      setMineruStarting(false);
    }
  };

  const handleBrowseOutputDir = async () => {
    const dir = await pickOutputDir();
    if (dir) setDefaultOutputDir(dir);
  };

  return (
    <div className="h-full overflow-auto">
      <div className="max-w-2xl mx-auto p-6 flex flex-col gap-6">
        <PageHeader title={t("settings.title")} />

        <Card>
          <CardHeader>
            <CardTitle>{t("settings.appearance")}</CardTitle>
            <CardDescription>{t("settings.theme")}</CardDescription>
          </CardHeader>
          <CardContent>
            <div className="grid grid-cols-3 gap-2">
              {themeOptions.map((opt) => (
                <button
                  key={opt.value}
                  onClick={() => setMode(opt.value)}
                  className={cn(
                    "flex items-center justify-center gap-2 rounded-md border px-3 py-2 text-sm font-medium transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 focus-visible:ring-offset-background",
                    theme === opt.value
                      ? "border-primary bg-accent text-accent-foreground"
                      : "border-input text-muted-foreground hover:bg-accent/50 hover:text-accent-foreground"
                  )}
                >
                  {opt.icon}
                  {t(opt.labelKey)}
                </button>
              ))}
            </div>
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle>{t("settings.conversion")}</CardTitle>
          </CardHeader>
          <CardContent>
            <div className="flex flex-col divide-y divide-border">
              <div className="py-2.5">
                <span className="text-sm text-muted-foreground block mb-2">
                  {t("settings.defaultOutputDir")}
                </span>
                <div className="flex gap-2">
                  <Input
                    type="text"
                    value={defaultOutputDir}
                    onChange={(e) => setDefaultOutputDir(e.target.value)}
                    placeholder={t("home.outputDirPlaceholder")}
                    className="flex-1"
                  />
                  <Button variant="outline" onClick={handleBrowseOutputDir}>
                    <FolderOpen size={14} />
                    {t("home.browse")}
                  </Button>
                </div>
              </div>
              <ToggleRow
                label={t("settings.recursive")}
                checked={recursive}
                onChange={setRecursive}
              />
              <ToggleRow
                label={t("settings.keepStructure")}
                checked={keepStructure}
                onChange={setKeepStructure}
              />
              <div className="py-2.5">
                <span className="text-sm text-muted-foreground block mb-2">
                  {t("settings.parseQuality")}
                </span>
                <div className="flex gap-2">
                  {(["auto", "quick", "high"] as ParseQuality[]).map((mode) => (
                    <button
                      key={mode}
                      onClick={() => setParseQuality(mode)}
                      className={cn(
                        "flex-1 rounded-md border px-3 py-1.5 text-sm font-medium transition-colors",
                        parseQuality === mode
                          ? "border-primary bg-accent text-accent-foreground"
                          : "border-input text-muted-foreground hover:bg-accent/50"
                      )}
                    >
                      {t(`settings.parseQualityMode.${mode}`)}
                    </button>
                  ))}
                </div>
                <span className="text-xs text-muted-foreground/70 mt-1.5 block">
                  {t("settings.parseQualityDesc")}
                </span>
              </div>
              <div className="py-2.5">
                <span className="text-sm text-muted-foreground block mb-2">
                  {t("settings.mineru")}
                </span>
                <div className="flex items-center justify-between gap-3 rounded-md border p-3">
                  <div className="flex flex-col gap-1 min-w-0">
                    <span className="text-sm flex items-center gap-2">
                      <Cpu size={14} className={cn(
                        "shrink-0",
                        mineruInfo?.healthy
                          ? "text-emerald-500"
                          : "text-muted-foreground"
                      )} />
                      {mineruInfo
                        ? mineruInfo.healthy
                          ? t("settings.mineruHealthy")
                          : t("settings.mineruUnhealthy")
                        : t("settings.mineruChecking")}
                    </span>
                    {mineruError && (
                      <span className="text-xs text-destructive break-all">
                        {mineruError}
                      </span>
                    )}
                  </div>
                  <Button
                    variant="outline"
                    disabled={mineruStarting || mineruInfo?.healthy}
                    onClick={handleStartMineru}
                  >
                    {mineruStarting
                      ? t("settings.mineruStarting")
                      : t("settings.mineruStart")}
                  </Button>
                </div>
                <span className="text-xs text-muted-foreground/70 mt-1.5 block">
                  {t("settings.mineruDesc")}
                </span>
              </div>
            </div>
            <div className="mt-4">
              <OutputModeSelector />
            </div>
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle className="flex items-center gap-2">
              <Sparkles size={16} />
              {t("settings.ai")}
            </CardTitle>
            <CardDescription>{t("settings.aiDesc")}</CardDescription>
          </CardHeader>
          <CardContent>
            <div className="flex flex-col divide-y divide-border">
              <ToggleRow
                label={t("settings.aiToggle")}
                checked={aiEnabled}
                onChange={setAiEnabled}
              />
              <ToggleRow
                label={t("settings.genToc")}
                checked={aiReadyToc}
                disabled={!aiEnabled}
                onChange={setAiReadyToc}
              />
              <ToggleRow
                label={t("settings.genMeta")}
                checked={aiReadyMeta}
                disabled={!aiEnabled}
                onChange={setAiReadyMeta}
              />
              <ToggleRow
                label={t("settings.allowOnline")}
                checked={false}
                disabled
                hint={t("common.comingSoon")}
              />
            </div>
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle className="flex items-center gap-2">
              <ShieldCheck size={16} />
              {t("settings.privacy")}
            </CardTitle>
          </CardHeader>
          <CardContent>
            <p className="text-sm text-muted-foreground leading-relaxed whitespace-pre-line">
              {t("settings.privacyNote")}
            </p>
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle>{t("settings.about")}</CardTitle>
          </CardHeader>
          <CardContent>
            <div className="flex flex-col divide-y divide-border">
              <InfoRow label={t("settings.version")}>v0.2.0</InfoRow>
              <InfoRow label={t("settings.techStack")}>
                React 18 · TypeScript · Tauri 2 · Tailwind CSS
              </InfoRow>
              <InfoRow label={t("settings.language")}>
                <span className="text-muted-foreground">
                  {t("settings.languageComingSoon")}
                </span>
              </InfoRow>
            </div>
          </CardContent>
        </Card>
      </div>
    </div>
  );
}