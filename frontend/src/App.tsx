import { useState } from "react";
import {
  ArrowLeft,
  FolderOpen,
  Home,
  ListTree,
  PanelLeft,
  Settings,
} from "lucide-react";
import { HomePage } from "./pages/HomePage";
import { ConvertPage } from "./pages/ConvertPage";
import { BatchPage } from "./pages/BatchPage";
import { useTaskStore } from "./store/useTaskStore";
import { cn } from "./lib/utils";

type Page = "home" | "convert" | "batch" | "settings";

export function App() {
  const [page, setPage] = useState<Page>("home");
  const [sidebarOpen, setSidebarOpen] = useState(true);
  const { currentTask } = useTaskStore();

  const navItems: { id: Page; label: string; icon: React.ReactNode }[] = [
    { id: "home", label: "Home", icon: <Home size={18} /> },
    { id: "convert", label: "Convert", icon: <ArrowLeft size={18} /> },
    { id: "batch", label: "Batch", icon: <ListTree size={18} /> },
    { id: "settings", label: "Settings", icon: <Settings size={18} /> },
  ];

  const renderPage = () => {
    switch (page) {
      case "home":
        return <HomePage onNavigate={setPage} />;
      case "convert":
        return <ConvertPage />;
      case "batch":
        return <BatchPage />;
      case "settings":
        return (
          <div className="flex items-center justify-center h-full text-muted-foreground">
            <div className="text-center">
              <Settings className="mx-auto mb-4 opacity-50" size={48} />
              <p className="text-lg font-medium">Settings</p>
              <p className="text-sm mt-2">Coming soon in Phase 2</p>
            </div>
          </div>
        );
      default:
        return <HomePage onNavigate={setPage} />;
    }
  };

  return (
    <div className="h-screen w-screen flex flex-col bg-white">
      <header className="h-12 border-b border-border bg-background px-4 flex items-center gap-3 shrink-0">
        <button
          onClick={() => setSidebarOpen(!sidebarOpen)}
          className="p-1.5 rounded-md hover:bg-slate-100 transition-colors"
        >
          <PanelLeft size={18} />
        </button>
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
            <span className="text-xs text-muted-foreground px-2 py-1 rounded bg-slate-50">
              Converting: {currentTask.sourcePath.split("/").pop()}
            </span>
          )}
        </div>
      </header>

      <div className="flex flex-1 overflow-hidden">
        {sidebarOpen && (
          <aside className="w-52 border-r border-border bg-slate-50/50 shrink-0 p-3 flex flex-col gap-1">
            <div className="mb-3 px-3 py-2">
              <button
                onClick={() => setPage("home")}
                className="w-full flex items-center gap-2 px-3 py-2 rounded-md hover:bg-slate-200 transition-colors text-sm"
              >
                <FolderOpen size={16} />
                <span>Files</span>
              </button>
            </div>
            <nav className="flex flex-col gap-0.5">
              {navItems.map((item) => (
                <button
                  key={item.id}
                  onClick={() => setPage(item.id)}
                  className={cn(
                    "flex items-center gap-2 px-3 py-2 rounded-md text-sm transition-colors",
                    page === item.id
                      ? "bg-violet-100 text-violet-700 font-medium"
                      : "hover:bg-slate-200 text-slate-600"
                  )}
                >
                  {item.icon}
                  <span>{item.label}</span>
                </button>
              ))}
            </nav>

            <div className="mt-auto px-3 py-3 border-t border-border">
              <div className="text-xs text-muted-foreground">
                <div className="flex justify-between mb-1">
                  <span>Phase 1 MVP</span>
                  <span>v0.1.0</span>
                </div>
                <div className="text-[10px] opacity-70">
                  anydoc + Tauri 2.x
                </div>
              </div>
            </div>
          </aside>
        )}

        <main className="flex-1 overflow-hidden">{renderPage()}</main>
      </div>
    </div>
  );
}
