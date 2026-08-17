import { useCallback, useEffect, useRef, useState } from "react";
import { writeTextFile } from "../api/tauriApi";
import { showToast } from "../lib/toast";

export function useAutoSave(
  content: string,
  filePath: string | null,
  delay = 1500,
): { saving: boolean; lastSaved: number | null; saveNow: () => Promise<void> } {
  const [saving, setSaving] = useState(false);
  const [lastSaved, setLastSaved] = useState<number | null>(null);
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const contentRef = useRef(content);
  const lastSavedContentRef = useRef(content);
  const filePathRef = useRef(filePath);
  filePathRef.current = filePath;
  contentRef.current = content;

  const saveNow = useCallback(async () => {
    const path = filePathRef.current;
    if (!path) return;
    const current = contentRef.current;
    if (current === lastSavedContentRef.current) return;
    setSaving(true);
    try {
      await writeTextFile(path, current);
      lastSavedContentRef.current = current;
      setLastSaved(Date.now());
    } catch (err) {
      showToast(`Save failed: ${err instanceof Error ? err.message : String(err)}`, 3000);
    } finally {
      setSaving(false);
    }
  }, []);

  useEffect(() => {
    if (timerRef.current) clearTimeout(timerRef.current);
    if (!filePath || content === lastSavedContentRef.current) return;
    timerRef.current = setTimeout(() => {
      saveNow();
    }, delay);
    return () => {
      if (timerRef.current) clearTimeout(timerRef.current);
    };
  }, [content, filePath, delay, saveNow]);

  useEffect(() => {
    return () => {
      if (timerRef.current) clearTimeout(timerRef.current);
    };
  }, []);

  const dirty = content !== lastSavedContentRef.current;

  return { saving, lastSaved, saveNow };
}