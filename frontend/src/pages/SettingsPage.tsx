import { useEffect, useState } from "react";
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
import { getAppVersion } from "../api/tauriApi";
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

interface NavSection {
  id: string;
  icon: React.ReactNode;
  labelKey: string;
}

const SECTIONS: NavSection[] = [
  { id: "appearance", icon: <Sun size={15} />, labelKey: "settings.appearance" },
  { id: "conversion", icon: <Cpu size={15} />, labelKey: "settings.conversion" },
  { id: "ai", icon: <Sparkles size={15} />, labelKey: "settings.ai" },
  { id: "privacy", icon: <ShieldCheck size={15} />, labelKey: "settings.privacy" },
  { id: "about", icon: <Monitor size={15} />, labelKey: "settings.about" },
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

function SettingsNav({
  activeId,
  onNavigate,
  t,
}: {
  activeId: string;
  onNavigate: (id: string) => void;
  t: (key: string) => string;
}) {
  return (
    <nav className="flex flex-col gap-0.5 sticky top-0">
      {SECTIONS.map((section) => (
        <button
          key={section.id}
          onClick={() => onNavigate(section.id)}
          className={cn(
            "flex items-center gap-2.5 px-3 py-2 rounded-lg text-sm font-medium text-left transition-all duration-200",
            activeId === section.id
              ? "bg-primary/8 text-primary shadow-sm"
              : "text-muted-foreground hover:bg-muted/50 hover:text-foreground"
          )}
        >
          <span className={cn(
            activeId === section.id ? "text-primary" : "text-muted-foreground/70"
          )}>
            {section.icon}
          </span>
          <span className="truncate">{t(section.labelKey)}</span>
        </button>
      ))}
    </nav>
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
    allowOnline,
    setDefaultOutputDir,
    setRecursive,
    setKeepStructure,
    setAiEnabled,
    setAiReadyToc,
    setAiReadyMeta,
    setAllowOnline,
  } = useSettingsStore();

  const [activeSection, setActiveSection] = useState("appearance");
  const [appVersion, setAppVersion] = useState<string>("v0.1.0");

  useEffect(() => {
    getAppVersion().then(setAppVersion).catch(() => {});
  }, []);

  const handleBrowseOutputDir = async () => {
    const dir = await pickOutputDir();
    if (dir) setDefaultOutputDir(dir);
  };

  const handleNavigate = (id: string) => {
    setActiveSection(id);
    const el = document.getElementById(id);
    if (el) {
      el.scrollIntoView({ behavior: "smooth", block: "start" });
    }
  };

  return (
    <div className="h-full overflow-hidden flex">
      <aside className="w-48 shrink-0 border-r border-border p-3 pt-4 overflow-y-auto">
        <SettingsNav activeId={activeSection} onNavigate={handleNavigate} t={t} />
      </aside>

      <div className="flex-1 min-w-0 overflow-y-auto">
        <div className="max-w-2xl mx-auto p-6 flex flex-col gap-5">
          <PageHeader title={t("settings.title")} />

          <div id="appearance">
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
                          ? "border-primary bg-accent text-accent-foreground shadow-sm"
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
          </div>

          <div id="conversion">
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
                              ? "border-primary bg-accent text-accent-foreground shadow-sm"
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
                      {t("settings.engineNotice")}
                    </span>
                    <span className="text-xs text-muted-foreground/70 mt-1.5 block">
                      {t("settings.engineNoticeDesc")}
                    </span>
                  </div>
                </div>
                <div className="mt-4">
                  <OutputModeSelector />
                </div>
              </CardContent>
            </Card>
          </div>

          <div id="ai">
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
                    checked={allowOnline}
                    onChange={setAllowOnline}
                    hint={t("settings.allowOnlineHint")}
                  />
                </div>
              </CardContent>
            </Card>
          </div>

          <div id="privacy">
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
          </div>

          <div id="about">
            <Card>
              <CardHeader>
                <CardTitle>{t("settings.about")}</CardTitle>
              </CardHeader>
              <CardContent>
                <div className="flex flex-col divide-y divide-border">
                  <InfoRow label={t("settings.version")}>{appVersion}</InfoRow>
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
      </div>
    </div>
  );
}
