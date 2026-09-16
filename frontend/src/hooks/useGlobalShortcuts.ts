import { useEffect, useRef } from "react";

/**
 * Global Ctrl/Cmd+<key> shortcuts.
 *
 * `enabled` lets callers suppress the shortcuts while a modal owns the UI
 * (e.g. the update dialog), so its key handling can't be hijacked.
 */
export function useGlobalShortcuts(
  handlers: Record<string, () => void>,
  enabled = true,
): void {
  const handlersRef = useRef(handlers);
  handlersRef.current = handlers;
  const enabledRef = useRef(enabled);
  enabledRef.current = enabled;

  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      const isMod = e.metaKey || e.ctrlKey;
      if (!isMod) return;
      if (!enabledRef.current) return;

      const key = e.key.toLowerCase();
      const shift = e.shiftKey;

      let shortcut = "";
      if (shift) shortcut += "Shift+";
      shortcut += key.toUpperCase();

      if (handlersRef.current[shortcut]) {
        const tag = (e.target as HTMLElement)?.tagName;
        const isInput =
          tag === "INPUT" ||
          tag === "TEXTAREA" ||
          !!(e.target as HTMLElement)?.closest?.(".cm-editor");
        if (isInput) return;
        e.preventDefault();
        e.stopPropagation();
        handlersRef.current[shortcut]();
      }
    };

    window.addEventListener("keydown", handler, { capture: true });
    return () => window.removeEventListener("keydown", handler, { capture: true });
  }, []);
}
