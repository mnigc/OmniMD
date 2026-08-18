import { useEffect, useState, useCallback } from "react";
import {
  Cloud,
  Eye,
  Clock,
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
import { Button } from "./components/ui/button";
import { cn } from "./lib/utils";
import {
  getAppVersion,
  getDefaultOutputDir,
  getActiveWorkspace,
  writeTextFile,
  scanWorkspace,
  startMineru,
} from "./api/tauriApi";
import { ToastPortal, showToast } from "./lib/toast";
import { useTaskStore } from "./store/useTaskStore";
import { useBatchStore } from "./store/useBatchStore";
import { useModelStore } from "./store/useModelStore";
import { useSettingsStore } from "./store/useSettingsStore";
import { BatchTaskPanel } from "./components/BatchTaskPanel";
import { ModelBanner } from "./components/ModelBanner";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { useGlobalShortcuts } from "./hooks/useGlobalShortcuts";
import type { ModelInfo } from "./types";

type Page = "home" | "library" | "convert" | "history" | "settings";

export function App() {
  const { t } = useI18n();
  const [page, setPage] = useState<Page>("home");
  const [sidebarOpen, setSidebarOpen] = useState(true);
  const [appVersion, setAppVersion] = useState("v0.1.0");
  const { engineMode, modelReady, downloading, downloadProgress } = useModelStore();
  const batchPanelOpen = useBatchStore((s) => s.panelOpen);
  const setBatchPanelOpen = useBatchStore((s) => s.setPanelOpen);

  useEffect(() => {
    getAppVersion().then(setAppVersion).catch(() => {});
  }, []);

  // Listen for shell-context-menu argv: convert files and auto-ingest into library.
  useEffect(() => {
    const win = getCurrentWebviewWindow();
    let unlisten: (() => void) | null = null;

    (async () => {
      try {
        unlisten = await win.listen<string[]>("argv-files", async (event) => {
          const files = event.payload.filter((f) => f.trim().length > 0);
          if (files.length === 0) return;

          const outputDir = await getDefaultOutputDir();
          const batchStore = useBatchStore.getState();
          const settingsStore = useSettingsStore.getState();
          for (const file of files) {
            const fileName = file.split(/[\\/]/).pop() || "output";
            const outputName = fileName.replace(/\.[^.]+$/, ".md");
            const outputPath = `${outputDir}/${outputName}`;
            await batchStore.enqueue(file, outputPath, settingsStore.outputMode, settingsStore.parseQuality);
          }
          await batchStore.refreshTasks();
          await batchStore.refreshSummary();
          showToast(
            `${files.length} file${files.length > 1 ? "s" : ""} queued for conversion`,
            2000,
          );
          await batchStore.start();
        });
      } catch {
        // Not running in Tauri
      }
    })();

    return () => {
      unlisten?.();
    };
  }, []);

  useEffect(() => {
    applyTheme();
    const cleanup = listenForSystemThemeChange(() => {
      if (getStoredTheme() === "auto") {
        applyTheme("auto");
      }
    });
    return cleanup;
  }, []);

  // First-launch check: whether the local pipeline model exists and which
  // engine mode is persisted. Also registers the model download progress
  // listener shared with the Settings page.
  useEffect(() => {
    let unlisten: (() => void) | null = null;
    (async () => {
      try {
        const store = useModelStore.getState();
        await store.refreshModelReady();
        await store.refreshEngineMode();
        unlisten = await store.listenForProgress();
      } catch {
        // Not running in Tauri
      }
    })();
    return () => {
      unlisten?.();
    };
  }, []);

  // React to pipeline model downloads that happen elsewhere (e.g. the
  // Settings page): mark the model ready and, if cloud mode was chosen only
  // because no local model existed, switch back to the local engine.
  useEffect(() => {
    const checkModelReady = (models: ModelInfo[]) => {
      const pipelineReady = models.some(
        (m) => m.name === "pipeline" && m.status === "downloaded"
      );
      if (!pipelineReady) return;
      useModelStore.getState().refreshModelReady();
      const state = useModelStore.getState();
      if (state.engineMode === "cloud") {
        state.setEngineModeAction("local");
      }
    };
    checkModelReady(useModelStore.getState().models);
    const unsub = useModelStore.subscribe((state) => checkModelReady(state.models));
    return unsub;
  }, []);

  const handleDownloadModel = useCallback(async () => {
    const store = useModelStore.getState();
    await store.downloadModel("pipeline");
    await store.setEngineModeAction("local");
    try {
      await startMineru();
    } catch {
      // best-effort: MinerUEngine auto-starts the runtime on first convert
    }
    await store.refreshModelReady();
  }, []);

  const handleUseCloud = useCallback(async () => {
    await useModelStore.getState().setEngineModeAction("cloud");
  }, []);

  // Close batch panel when page changes to avoid stale event listeners
  useEffect(() => {
    setBatchPanelOpen(false);
  }, [page, setBatchPanelOpen]);

  const handleNewMarkdown = useCallback(async () => {
    const ws = await getActiveWorkspace();
    if (!ws) { showToast(t("editor.newFileHint")); return; }
    const name = `untitled-${Date.now()}.md`;
    const path = `${ws.path}/${name}`;
    try {
      await writeTextFile(path, "# Untitled\n\n");
      await scanWorkspace(ws.id);
      showToast(t("editor.newFileCreated"));
    } catch {
      showToast("Failed to create file", 3000);
    }
  }, [t]);

  useGlobalShortcuts({
    O: () => { setPage("home"); },
    N: () => handleNewMarkdown(),
    P: () => { setPage("library"); },
    "Shift+F": () => { setPage("library"); },
  });

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
          {engineMode === "cloud" && (
            <span
              className="flex items-center gap-1.5 text-xs text-muted-foreground px-2 py-1 rounded-md border border-sky-300/40 bg-sky-500/10"
              title={t("banner.cloudMode")}
            >
              <Cloud size={13} className="text-sky-500" />
              {t("banner.cloudMode")}
            </span>
          )}
          <Button
            variant="ghost"
            size="sm"
            onClick={() => {
              setBatchPanelOpen(true);
              useBatchStore.getState().refreshTasks();
              useBatchStore.getState().refreshSummary();
            }}
            className="text-xs gap-1.5"
          >
            {t("batch.title")}
          </Button>
        </div>
        <WindowControls />
      </header>

      {!modelReady && engineMode !== "cloud" && (
        <ModelBanner
          downloading={downloading}
          progress={downloadProgress.pipeline?.progress ?? 0}
          onDownload={handleDownloadModel}
          onUseCloud={handleUseCloud}
          onCancelDownload={() => {
            useModelStore.getState().cancelDownload();
          }}
        />
      )}

      <div className="flex flex-1 overflow-hidden">
        <aside
          className={cn(
            "shrink-0 border-r border-border bg-muted/40 p-3 flex flex-col gap-1 overflow-hidden transition-all duration-250 ease-out",
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
            <div className={cn("px-3 pt-2 border-t border-border text-xs text-muted-foreground", !sidebarOpen && "hidden")}>
              <div className="flex justify-between mb-0.5">
                <span>{t("home.phase1Mvp")}</span>
                <span>{appVersion}</span>
              </div>
              <div className="opacity-70 truncate">{t("home.minerUTauri")}</div>
            </div>
          </div>
        </aside>

        <main className="flex-1 overflow-hidden">{renderPage()}</main>
      </div>
      <BatchTaskPanel
        open={batchPanelOpen}
        onClose={() => setBatchPanelOpen(false)}
      />
      <ToastPortal />
    </div>
  );
}