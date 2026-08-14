import { Monitor, Moon, Sun } from "lucide-react";
import { useI18n } from "../i18n";
import { type ThemeMode } from "../lib/theme";
import { useThemeMode } from "../hooks/useThemeMode";
import { PageHeader } from "../components/PageHeader";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "../components/ui/card";
import { cn } from "../lib/utils";

const themeOptions: {
  value: ThemeMode;
  icon: React.ReactNode;
  labelKey: string;
}[] = [
  { value: "light", icon: <Sun size={16} />, labelKey: "theme.light" },
  { value: "dark", icon: <Moon size={16} />, labelKey: "theme.dark" },
  { value: "auto", icon: <Monitor size={16} />, labelKey: "theme.auto" },
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

export function SettingsPage() {
  const { t } = useI18n();
  const { theme, setMode } = useThemeMode();

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
            <CardTitle>{t("settings.about")}</CardTitle>
          </CardHeader>
          <CardContent>
            <div className="flex flex-col divide-y divide-border">
              <InfoRow label={t("settings.version")}>v0.1.0</InfoRow>
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