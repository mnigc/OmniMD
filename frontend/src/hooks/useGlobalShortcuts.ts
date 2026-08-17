import { useEffect, useRef } from "react";

export function useGlobalShortcuts(handlers: Record<string, () => void>): void {
  const handlersRef = useRef(handlers);
  handlersRef.current = handlers;

  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      const isMod = e.metaKey || e.ctrlKey;
      if (!isMod) return;

      const key = e.key.toLowerCase();
      const shift = e.shiftKey;

      let shortcut = "";
      if (shift) shortcut += "Shift+";
      shortcut += key.toUpperCase();

      if (handlersRef.current[shortcut]) {
        if (shortcut !== "S") {
          const tag = (e.target as HTMLElement)?.tagName;
          const isInput = tag === "INPUT" || tag === "TEXTAREA" || !!(e.target as HTMLElement)?.closest?.(".cm-editor");
          if (isInput) return;
        }
        e.preventDefault();
        e.stopPropagation();
        handlersRef.current[shortcut]();
      }
    };

    window.addEventListener("keydown", handler, { capture: true });
    return () => window.removeEventListener("keydown", handler, { capture: true });
  }, []);
}