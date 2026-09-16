import { useEffect, useState } from "react";
import { Copy, Minus, Square, X } from "lucide-react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { cn } from "../lib/utils";

function isWindows() {
  const ua = navigator.userAgent;
  return ua.includes("Windows") || ua.includes("Win64");
}

const baseButtonClass =
  "flex h-full w-11 items-center justify-center text-muted-foreground transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring";

export function WindowControls() {
  const [isMaximized, setIsMaximized] = useState(false);

  useEffect(() => {
    const win = getCurrentWindow();
    let unlisten: (() => void) | undefined;
    // 组件可能在 await 完成前卸载：置脏标记防止注销前注册的监听泄漏。
    let cleanedUp = false;

    (async () => {
      try {
        setIsMaximized(await win.isMaximized());
        const fn = await win.onResized(() => {
          win.isMaximized().then(setIsMaximized);
        });
        if (cleanedUp) fn();
        else unlisten = fn;
      } catch (err) {
        console.error("Failed to init window controls", err);
      }
    })();

    return () => {
      cleanedUp = true;
      unlisten?.();
    };
  }, []);

  if (!isWindows()) return null;

  // Resolve the window handle lazily inside handlers: calling getCurrentWindow()
  // during render throws when the Tauri runtime is absent (e.g. dev preview in a
  // plain browser) and would crash the whole tree through the error boundary.
  const withWindow = (fn: (win: ReturnType<typeof getCurrentWindow>) => void) => {
    try {
      fn(getCurrentWindow());
    } catch {
      // Not running in Tauri
    }
  };

  return (
    <div className="flex h-full items-stretch">
      <button
        type="button"
        aria-label="Minimize"
        onClick={() => withWindow((win) => win.minimize().catch(() => {}))}
        className={cn(baseButtonClass, "hover:bg-muted/80 hover:text-foreground")}
      >
        <Minus size={14} />
      </button>
      <button
        type="button"
        aria-label={isMaximized ? "Restore" : "Maximize"}
        onClick={() => withWindow((win) => win.toggleMaximize().catch(() => {}))}
        className={cn(baseButtonClass, "hover:bg-muted/80 hover:text-foreground")}
      >
        {isMaximized ? <Copy size={14} /> : <Square size={14} />}
      </button>
      <button
        type="button"
        aria-label="Close"
        onClick={() => withWindow((win) => win.close().catch(() => {}))}
        className={cn(baseButtonClass, "hover:bg-destructive hover:text-white")}
      >
        <X size={14} />
      </button>
    </div>
  );
}