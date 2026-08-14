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

    (async () => {
      try {
        setIsMaximized(await win.isMaximized());
        unlisten = await win.onResized(() => {
          win.isMaximized().then(setIsMaximized);
        });
      } catch (err) {
        console.error("Failed to init window controls", err);
      }
    })();

    return () => {
      unlisten?.();
    };
  }, []);

  if (!isWindows()) return null;

  const win = getCurrentWindow();

  return (
    <div className="flex h-full items-stretch">
      <button
        type="button"
        aria-label="Minimize"
        onClick={() => win.minimize().catch(() => {})}
        className={cn(baseButtonClass, "hover:bg-muted/80 hover:text-foreground")}
      >
        <Minus size={14} />
      </button>
      <button
        type="button"
        aria-label={isMaximized ? "Restore" : "Maximize"}
        onClick={() => win.toggleMaximize().catch(() => {})}
        className={cn(baseButtonClass, "hover:bg-muted/80 hover:text-foreground")}
      >
        {isMaximized ? <Copy size={14} /> : <Square size={14} />}
      </button>
      <button
        type="button"
        aria-label="Close"
        onClick={() => win.close().catch(() => {})}
        className={cn(baseButtonClass, "hover:bg-red-600 hover:text-white")}
      >
        <X size={14} />
      </button>
    </div>
  );
}