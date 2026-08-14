import { useEffect, useState } from "react";
import {
  ArrowRightLeft,
  Home,
  ListTree,
  PanelLeft,
  Settings,
} from "lucide-react";
import { HomePage } from "./pages/HomePage";
import { ConvertPage } from "./pages/ConvertPage";
import { BatchPage } from "./pages/BatchPage";
import { SettingsPage } from "./pages/SettingsPage";
import { useTaskStore } from "./store/useTaskStore";
import { useI18n } from "./i18n";
import {
  applyTheme,
  getStoredTheme,
  listenForSystemThemeChange,
} from "./lib/theme";
import { SidebarNavItem } from "./components/SidebarNavItem";
import { WindowControls } from "./components/WindowControls";
import { Button } from "./components/ui/button";
import { cn } from "./lib/utils";

type Page = "home" | "convert" | "batch" | "settings";

export function App() {
  const { t } = useI18n();
  const [page, setPage] = useState<Page>("home");
  const [sidebarOpen, setSidebarOpen] = useState(true);
  const { currentTask } = useTaskStore();

  useEffect(() => {
    applyTheme();
    const cleanup = listenForSystemThemeChange(() => {
      if (getStoredTheme() === "auto") {
        applyTheme("auto");
      }
    });
    return cleanup;
  }, []);

  const renderPage = () => {
    switch (page) {
      case "home":
        return <HomePage onNavigate={setPage} />;
      case "convert":
        return <ConvertPage onNavigate={setPage} />;
      case "batch":
        return <BatchPage />;
      case "settings":
        return <SettingsPage />;
      default:
        return <HomePage onNavigate={setPage} />;
    }
  };

  return (
    <div className="h-screen w-screen flex flex-col bg-background text-foreground">
      <header
        data-tauri-drag-region="deep"
        className="h-12 bg-background/95 backdrop-blur border-b border-border pl-4 pr-0 flex items-center gap-3 shrink-0 select-none"
      >
        <Button
          variant="ghost"
          size="icon"
          onClick={() => setSidebarOpen(!sidebarOpen)}
          aria-label="Toggle sidebar"
        >
          <PanelLeft size={18} />
        </Button>
        <div className="flex items-center gap-2">
          <div className="w-7 h-7 rounded-md bg-gradient-to-br from-violet-500 to-blue-500 flex items-center justify-center">
            <span className="text-white font-bold text-xs">OM</span>
          </div>
          <span className="font-semibold text-sm">
            OmniMD - Anything to Markdown
          </span>
        </div>
        <div className="ml-auto flex items-center gap-2">
          {currentTask && (
            <span className="text-xs text-muted-foreground px-2 py-1 rounded-md bg-muted max-w-44 truncate">
              {t("header.converting")}:{" "}
              {currentTask.sourcePath.split("/").pop()}
            </span>
          )}
          </div>
        <WindowControls />
      </header>

      <div className="flex flex-1 overflow-hidden">
        <aside
          className={cn(
            "shrink-0 border-r border-border bg-muted/40 p-3 flex flex-col gap-1 overflow-hidden transition-all duration-250 ease-out",
            sidebarOpen ? "w-52" : "w-0"
          )}
        >
          <nav className="flex flex-col gap-0.5 flex-shrink-0">
            <SidebarNavItem
              icon={<Home size={16} />}
              label={t("nav.home")}
              active={page === "home"}
              onClick={() => setPage("home")}
            />
            <SidebarNavItem
              icon={<ArrowRightLeft size={16} />}
              label={t("nav.convert")}
              active={page === "convert"}
              onClick={() => setPage("convert")}
            />
            <SidebarNavItem
              icon={<ListTree size={16} />}
              label={t("nav.batch")}
              active={page === "batch"}
              onClick={() => setPage("batch")}
            />
          </nav>

          <div className="mt-auto flex flex-col gap-0.5 flex-shrink-0">
            <SidebarNavItem
              icon={<Settings size={16} />}
              label={t("nav.settings")}
              active={page === "settings"}
              onClick={() => setPage("settings")}
            />
            <div className="px-3 pt-2 border-t border-border text-xs text-muted-foreground">
              <div className="flex justify-between mb-0.5">
                <span>{t("home.phase1Mvp")}</span>
                <span>v0.1.0</span>
              </div>
              <div className="opacity-70 truncate">{t("home.anydocTauri")}</div>
            </div>
          </div>
        </aside>

        <main className="flex-1 overflow-hidden">{renderPage()}</main>
      </div>
    </div>
  );
}