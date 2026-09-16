import { useEffect, useState, useCallback } from "react";
import {
  Eye,
  Clock,
  Download,
  Home,
  LibraryBig,
  PanelLeft,
  Settings,
} from "lucide-react";
import { HomePage } from "./pages/HomePage";
import { ConvertPage } from "./pages/ConvertPage";
import { HistoryPage } from "./pages/HistoryPage";
import { LibraryPage } from "./pages/LibraryPage";
import { SettingsPage } from "./pages/SettingsPage";
import { useI18n } from "./i18n";
import {
  applyTheme,
  getStoredTheme,
  listenForSystemThemeChange,
} from "./lib/theme";
import { SidebarNavItem } from "./components/SidebarNavItem";
import { WindowControls } from "./components/WindowControls";
import { ErrorBoundary } from "./components/ErrorBoundary";
import { UpdateDialog } from "./components/UpdateDialog";
import { Button } from "./components/ui/button";
import { cn } from "./lib/utils";
import {
  getAppVersion,
  getDefaultOutputDir,
  getActiveWorkspace,
  writeTextFile,
  scanWorkspace,
} from "./api/tauriApi";
import { ToastPortal, showToast } from "./lib/toast";
import { useBatchStore } from "./store/useBatchStore";
import { useSettingsStore } from "./store/useSettingsStore";
import { useUpdateStore } from "./store/useUpdateStore";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { useGlobalShortcuts } from "./hooks/useGlobalShortcuts";

type Page = "home" | "library" | "convert" | "history" | "settings";

export function App() {
  const { t } = useI18n();
  const [page, setPage] = useState<Page>("home");
  const sidebarDefaultOpen = useSettingsStore((s) => s.sidebarDefaultOpen);
  const [sidebarOpen, setSidebarOpen] = useState(sidebarDefaultOpen);
  const [appVersion, setAppVersion] = useState("v0.1.0");

  useEffect(() => {
    getAppVersion().then(setAppVersion).catch(() => {});
  }, []);

  // The "sidebar default state" setting drives the startup state; applying it
  // live keeps the preview in Settings in sync with the actual sidebar.
  useEffect(() => {
    setSidebarOpen(sidebarDefaultOpen);
  }, [sidebarDefaultOpen]);

  const updateStatus = useUpdateStore((s) => s.status);
  const updateDialogOpen = useUpdateStore((s) => s.dialogOpen);
  const updateVersion = useUpdateStore((s) => s.version);
  const openUpdateDialog = useUpdateStore((s) => s.openDialog);

  // Check for a new release a few seconds after startup so the network round
  // trip never delays first paint. Failures stay silent (see the store).
  useEffect(() => {
    const timer = setTimeout(() => {
      void useUpdateStore.getState().check({ silent: true });
    }, 4000);
    return () => clearTimeout(timer);
  }, []);

  const showUpdateHint =
    updateStatus === "available" || updateStatus === "ready";

  // Listen for shell-context-menu argv: convert files and auto-ingest into library.
  useEffect(() => {
    const win = getCurrentWebviewWindow();
    let unlisten: (() => void) | null = null;
    let cleanedUp = false;

    (async () => {
      try {
        const un = await win.listen<string[]>("argv-files", async (event) => {
          const files = event.payload.filter((f) => f.trim().length > 0);
          if (files.length === 0) return;

          try {
            const outputDir = await getDefaultOutputDir();
            const batchStore = useBatchStore.getState();
            let queued = 0;
            for (const file of files) {
              const fileName = file.split(/[\\/]/).pop() || "output";
              const outputName = fileName.replace(/\.[^.]+$/, ".md");
              const outputPath = `${outputDir}/${outputName}`;
              if (await batchStore.enqueue(file, outputPath)) queued++;
            }
            await batchStore.refreshTasks();
            await batchStore.refreshSummary();
            showToast(t("app.queuedFiles", { count: queued }), 2000);
            await batchStore.start();
          } catch (err) {
            showToast(
              err instanceof Error ? err.message : t("app.crashed"),
              3000,
              "error",
            );
          }
        });
        // The effect may have been cleaned up while we awaited the listener.
        if (cleanedUp) un();
        else unlisten = un;
      } catch {
        // Not running in Tauri
      }
    })();

    return () => {
      cleanedUp = true;
      unlisten?.();
    };
  }, [t]);

  useEffect(() => {
    applyTheme();
    const cleanup = listenForSystemThemeChange(() => {
      if (getStoredTheme() === "auto") {
        applyTheme("auto");
      }
    });
    return cleanup;
  }, []);

  // Register batch-task event listeners once at the app level so HomePage and
  // other pages receive live status/progress updates even when the batch panel
  // is closed.
  useEffect(() => {
    let cleanup: (() => void) | null = null;
    let cleanedUp = false;
    (async () => {
      try {
        const c = await useBatchStore.getState().listenForEvents();
        if (cleanedUp) c();
        else cleanup = c;
      } catch {
        // Not running in Tauri
      }
    })();
    return () => {
      cleanedUp = true;
      cleanup?.();
    };
  }, []);

  // Sync the persisted concurrency setting to the backend on startup.
  useEffect(() => {
    const stored = useSettingsStore.getState().concurrency;
    useBatchStore.getState().setConcurrency(stored);
  }, []);

  // Load persisted batch tasks from the DB on startup so the queue is not
  // empty after a restart.
  useEffect(() => {
    useBatchStore.getState().refreshTasks();
    useBatchStore.getState().refreshSummary();
  }, []);

  const handleNewMarkdown = useCallback(async () => {
    try {
      const ws = await getActiveWorkspace();
      if (!ws) {
        showToast(t("editor.newFileHint"));
        return;
      }
      const name = `untitled-${Date.now()}.md`;
      const path = `${ws.path}/${name}`;
      await writeTextFile(path, "# Untitled\n\n");
      await scanWorkspace(ws.id);
      showToast(t("editor.newFileCreated"));
    } catch (err) {
      showToast(
        err instanceof Error ? err.message : t("app.createFileFailed"),
        3000,
        "error",
      );
    }
  }, [t]);

  useGlobalShortcuts(
    {
      O: () => {
        setPage("home");
      },
      N: () => handleNewMarkdown(),
      P: () => {
        setPage("library");
      },
      "Shift+F": () => {
        setPage("library");
      },
    },
    // While the update dialog is modal, its own keys (Escape/Tab) must not be
    // hijacked by global shortcuts.
    !updateDialogOpen,
  );

  const renderPage = () => {
    switch (page) {
      case "home":
        return <HomePage />;
      case "library":
        return <LibraryPage />;
      case "convert":
        return <ConvertPage onNavigate={setPage} />;
      case "history":
        return <HistoryPage onNavigate={setPage} />;
      case "settings":
        return <SettingsPage />;
      default:
        return <HomePage />;
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
          aria-label={t("app.toggleSidebar")}
        >
          <PanelLeft size={18} />
        </Button>
        <div className="flex items-center gap-2.5">
          <div className="w-7 h-7 rounded-lg bg-gradient-to-br from-violet-500 via-violet-600 to-indigo-600 ring-1 ring-white/25 shadow-sm flex items-center justify-center">
            <span className="text-white font-bold text-[11px] tracking-wide">OM</span>
          </div>
          <span className="font-semibold text-sm tracking-tight">
            OmniMD
            <span className="font-normal text-muted-foreground hidden lg:inline">
              {"  "}·{"  "}Anything to Markdown
            </span>
          </span>
        </div>
        <div className="ml-auto flex h-full items-stretch">
          <WindowControls />
        </div>
      </header>

      <div className="flex flex-1 overflow-hidden">
        <aside
          className={cn(
            "shrink-0 border-r border-border bg-muted/30 p-2.5 flex flex-col gap-1 overflow-hidden",
            "transition-[width,padding] duration-300 ease-[cubic-bezier(0.4,0,0.2,1)]",
            sidebarOpen ? "w-52" : "w-14"
          )}
        >
          <nav className="flex flex-col gap-0.5 flex-shrink-0">
            <SidebarNavItem
              icon={<Home size={16} />}
              label={t("nav.home")}
              active={page === "home"}
              onClick={() => setPage("home")}
              collapsed={!sidebarOpen}
            />
            <SidebarNavItem
              icon={<LibraryBig size={16} />}
              label={t("nav.library")}
              active={page === "library"}
              onClick={() => setPage("library")}
              collapsed={!sidebarOpen}
            />
            <SidebarNavItem
              icon={<Clock size={16} />}
              label={t("nav.history")}
              active={page === "history"}
              onClick={() => setPage("history")}
              collapsed={!sidebarOpen}
            />
            <SidebarNavItem
              icon={<Eye size={16} />}
              label={t("nav.convert")}
              active={page === "convert"}
              onClick={() => setPage("convert")}
              collapsed={!sidebarOpen}
            />
          </nav>

          <div className="mt-auto flex flex-col gap-0.5 flex-shrink-0">
            <SidebarNavItem
              icon={<Settings size={16} />}
              label={t("nav.settings")}
              active={page === "settings"}
              onClick={() => setPage("settings")}
              collapsed={!sidebarOpen}
            />
            {showUpdateHint && (
              <button
                type="button"
                onClick={openUpdateDialog}
                title={
                  updateVersion
                    ? t("update.available", { version: updateVersion })
                    : t("update.title")
                }
                aria-label={
                  updateVersion
                    ? t("update.available", { version: updateVersion })
                    : t("update.title")
                }
                className={cn(
                  "flex items-center gap-2 rounded-lg text-xs font-medium text-primary transition-colors hover:bg-primary/10",
                  sidebarOpen ? "px-3 py-2" : "justify-center px-0 py-2"
                )}
              >
                <Download size={14} className="shrink-0" />
                {sidebarOpen && (
                  <span className="truncate">
                    {updateStatus === "ready"
                      ? t("update.restartHint")
                      : t("update.sidebarHint", {
                          version: updateVersion ?? "",
                        })}
                  </span>
                )}
              </button>
            )}
            <div
              className={cn(
                "pt-2.5 mt-1 border-t border-border/70 text-[11px] text-muted-foreground/70 text-center",
                !sidebarOpen && "hidden"
              )}
            >
              <span className="tabular-nums">{appVersion}</span>
            </div>
          </div>
        </aside>

        <main className="flex-1 overflow-hidden">
          <ErrorBoundary
            resetKey={page}
            title={t("app.crashed")}
            retryLabel={t("common.retry")}
          >
            <div className="h-full w-full page-transition page-glow">{renderPage()}</div>
          </ErrorBoundary>
        </main>
      </div>
      <ToastPortal />
      <UpdateDialog />
    </div>
  );
}
