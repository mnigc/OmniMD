import { useEffect, useRef, useState } from "react";
import { Monitor, Moon, Sun, ShieldCheck, FolderOpen, Cpu, Loader2 } from "lucide-react";
import { useI18n, type Locale } from "../i18n";
import { type ThemeMode } from "../lib/theme";
import { useThemeMode } from "../hooks/useThemeMode";
import { PageHeader } from "../components/PageHeader";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "../components/ui/card";
import { Input } from "../components/ui/input";
import { Button } from "../components/ui/button";
import { cn } from "../lib/utils";
import { useSettingsStore } from "../store/useSettingsStore";
import { useUpdateStore } from "../store/useUpdateStore";
import { getAppVersion } from "../api/tauriApi";
import { pickOutputDir } from "../api/dialogs";
import { showToast } from "../lib/toast";

const themeOptions: {
  value: ThemeMode;
  icon: React.ReactNode;
  labelKey: string;
}[] = [
  { value: "light", icon: <Sun size={16} />, labelKey: "theme.light" },
  { value: "dark", icon: <Moon size={16} />, labelKey: "theme.dark" },
  { value: "auto", icon: <Monitor size={16} />, labelKey: "theme.auto" },
];

const languageOptions: { value: Locale; label: string }[] = [
  { value: "zh-CN", label: "简体中文" },
  { value: "en", label: "English" },
];

interface NavSection {
  id: string;
  icon: React.ReactNode;
  labelKey: string;
}

const SECTIONS: NavSection[] = [
  { id: "appearance", icon: <Sun size={15} />, labelKey: "settings.appearance" },
  { id: "conversion", icon: <Cpu size={15} />, labelKey: "settings.conversion" },
  { id: "privacy", icon: <ShieldCheck size={15} />, labelKey: "settings.privacy" },
  { id: "about", icon: <Monitor size={15} />, labelKey: "settings.about" },
];

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
    <nav className="flex flex-col gap-0.5">
      {SECTIONS.map((section) => (
        <button
          key={section.id}
          onClick={() => onNavigate(section.id)}
          aria-current={activeId === section.id ? "true" : undefined}
          className={cn(
            "flex items-center gap-2.5 px-3 py-2 rounded-lg text-sm font-medium text-left transition-all duration-200",
            activeId === section.id
              ? "bg-primary/10 text-primary shadow-sm"
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
  const { t, locale, setLocale } = useI18n();
  const { theme, setMode } = useThemeMode();
  const { defaultOutputDir, setDefaultOutputDir, sidebarDefaultOpen, setSidebarDefaultOpen } =
    useSettingsStore();

  const updateStatus = useUpdateStore((s) => s.status);
  const updateVersion = useUpdateStore((s) => s.version);
  const checkUpdate = useUpdateStore((s) => s.check);
  const openUpdateDialog = useUpdateStore((s) => s.openDialog);

  const updateBusy = updateStatus === "checking";
  const updateMessage =
    updateStatus === "up-to-date"
      ? t("update.upToDate")
      : updateStatus === "available"
        ? t("update.available", { version: updateVersion ?? "" })
        : updateStatus === "downloading"
          ? t("update.sidebarDownloading")
          : updateStatus === "downloaded"
            ? t("update.sidebarDownloaded")
            : updateStatus === "ready"
              ? t("update.restartHint")
              : updateStatus === "error"
                ? t("update.failed")
                : "";

  // When an update is already known (or in flight), the button opens the
  // dialog instead of re-running the check.
  const handleCheckUpdate = () => {
    if (
      updateStatus === "available" ||
      updateStatus === "downloading" ||
      updateStatus === "downloaded" ||
      updateStatus === "ready"
    ) {
      openUpdateDialog();
      return;
    }
    void checkUpdate();
  };

  const [activeSection, setActiveSection] = useState("appearance");
  const [appVersion, setAppVersion] = useState<string>("v0.1.0");
  const [outputDirInput, setOutputDirInput] = useState(defaultOutputDir);
  const scrollRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    getAppVersion().then(setAppVersion).catch(() => {});
  }, []);

  // Keep the text field in sync if the store changes elsewhere (e.g. HomePage).
  useEffect(() => {
    setOutputDirInput(defaultOutputDir);
  }, [defaultOutputDir]);

  // Scroll-spy: highlight the section currently in view.
  useEffect(() => {
    const root = scrollRef.current;
    if (!root) return;
    const observer = new IntersectionObserver(
      (entries) => {
        for (const entry of entries) {
          if (entry.isIntersecting) setActiveSection(entry.target.id);
        }
      },
      { root, rootMargin: "-20% 0px -70% 0px", threshold: 0 }
    );
    for (const section of SECTIONS) {
      const el = document.getElementById(section.id);
      if (el) observer.observe(el);
    }
    return () => observer.disconnect();
  }, []);

  const handleBrowseOutputDir = async () => {
    try {
      const dir = await pickOutputDir(t("settings.defaultOutputDir"));
      if (dir) {
        setDefaultOutputDir(dir);
        setOutputDirInput(dir);
      }
    } catch (err) {
      showToast(
        err instanceof Error ? err.message : t("toast.folderPickFailed"),
        3000,
        "error"
      );
    }
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

      <div ref={scrollRef} className="flex-1 min-w-0 overflow-y-auto">
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
                      aria-pressed={theme === opt.value}
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

                <div className="mt-3 pt-3 border-t border-border">
                  <InfoRow label={t("settings.sidebarDefault")}>
                    <div className="inline-flex rounded-lg bg-muted p-0.5">
                      {(
                        [
                          { value: true, labelKey: "settings.sidebarExpanded" },
                          { value: false, labelKey: "settings.sidebarCollapsed" },
                        ] as const
                      ).map((opt) => (
                        <button
                          key={String(opt.value)}
                          type="button"
                          onClick={() => setSidebarDefaultOpen(opt.value)}
                          aria-pressed={sidebarDefaultOpen === opt.value}
                          className={cn(
                            "h-7 px-3 rounded-md text-xs font-medium transition-colors",
                            sidebarDefaultOpen === opt.value
                              ? "bg-background text-foreground shadow-sm"
                              : "text-muted-foreground hover:text-foreground"
                          )}
                        >
                          {t(opt.labelKey)}
                        </button>
                      ))}
                    </div>
                  </InfoRow>
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
                        value={outputDirInput}
                        onChange={(e) => setOutputDirInput(e.target.value)}
                        onBlur={() => setDefaultOutputDir(outputDirInput)}
                        placeholder={t("home.outputDirPlaceholder")}
                        className="flex-1"
                      />
                      <Button variant="outline" onClick={handleBrowseOutputDir}>
                        <FolderOpen size={14} />
                        {t("home.browse")}
                      </Button>
                    </div>
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
                  <InfoRow label={t("update.label")}>
                    <div className="flex items-center justify-end gap-2">
                      {updateMessage && (
                        <span className="text-xs text-muted-foreground">
                          {updateMessage}
                        </span>
                      )}
                      <Button
                        variant="outline"
                        size="sm"
                        onClick={handleCheckUpdate}
                        disabled={updateBusy}
                      >
                        {updateBusy && <Loader2 size={12} className="animate-spin" />}
                        {updateBusy ? t("update.checking") : t("update.checkNow")}
                      </Button>
                    </div>
                  </InfoRow>
                  <InfoRow label={t("settings.techStack")}>
                    React 18 · TypeScript · Tauri 2 · Tailwind CSS
                  </InfoRow>
                  <InfoRow label={t("settings.language")}>
                    <div className="inline-flex rounded-lg bg-muted p-0.5">
                      {languageOptions.map((opt) => (
                        <button
                          key={opt.value}
                          type="button"
                          onClick={() => setLocale(opt.value)}
                          aria-pressed={locale === opt.value}
                          className={cn(
                            "h-7 px-3 rounded-md text-xs font-medium transition-colors",
                            locale === opt.value
                              ? "bg-background text-foreground shadow-sm"
                              : "text-muted-foreground hover:text-foreground"
                          )}
                        >
                          {opt.label}
                        </button>
                      ))}
                    </div>
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
