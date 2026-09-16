import { useCallback, useEffect, useRef, useState } from "react";
import { writeTextFile } from "../api/tauriApi";
import { showToast } from "../lib/toast";
import { useI18n } from "../i18n";

/**
 * Debounced autosave for a single edited file.
 *
 * Correctness guarantees:
 * - Pending edits are flushed to their ORIGINAL path before `filePath` changes,
 *   so switching documents never silently drops the previous file's changes.
 * - The saved baseline is per-file: the first content delivered for a newly
 *   selected file is treated as its baseline, not as a pending edit (which
 *   would otherwise write one document's text into another file).
 */
export function useAutoSave(
  content: string,
  filePath: string | null,
  delay = 1500,
): { saving: boolean; lastSaved: number | null; saveNow: () => Promise<void> } {
  const { t } = useI18n();
  const [saving, setSaving] = useState(false);
  const [lastSaved, setLastSaved] = useState<number | null>(null);
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const contentRef = useRef(content);
  const filePathRef = useRef<string | null>(filePath);
  const baselineRef = useRef(content);
  const pendingRef = useRef<{ path: string; content: string } | null>(null);
  const awaitingLoadRef = useRef(true);

  contentRef.current = content;

  const flush = useCallback(async () => {
    if (timerRef.current) {
      clearTimeout(timerRef.current);
      timerRef.current = null;
    }
    const pending = pendingRef.current;
    if (!pending) return;
    pendingRef.current = null;

    setSaving(true);
    try {
      await writeTextFile(pending.path, pending.content);
      if (pending.path === filePathRef.current) {
        baselineRef.current = pending.content;
        setLastSaved(Date.now());
      }
    } catch (err) {
      // Keep the pending edit so a later attempt can retry it.
      pendingRef.current = pending;
      showToast(
        t("editor.saveFailed", {
          error: err instanceof Error ? err.message : String(err),
        }),
        3000,
        "error",
      );
    } finally {
      setSaving(false);
    }
  }, [t]);

  const saveNow = useCallback(() => flush(), [flush]);

  // Runs BEFORE the scheduling effect on a file switch (declaration order).
  useEffect(() => {
    const prev = filePathRef.current;
    if (prev === filePath) return;
    // Flush edits that belong to the file we are leaving.
    if (pendingRef.current && pendingRef.current.path === prev) {
      void flush();
    }
    pendingRef.current = null;
    filePathRef.current = filePath;
    awaitingLoadRef.current = true;
  }, [filePath, flush]);

  useEffect(() => {
    if (!filePath) return;
    if (awaitingLoadRef.current) {
      // First content for this file is its baseline, not an edit.
      baselineRef.current = content;
      awaitingLoadRef.current = false;
      return;
    }
    if (content === baselineRef.current) return;
    pendingRef.current = { path: filePath, content };
    if (timerRef.current) clearTimeout(timerRef.current);
    timerRef.current = setTimeout(() => {
      void flush();
    }, delay);
    return () => {
      if (timerRef.current) clearTimeout(timerRef.current);
    };
  }, [content, filePath, delay, flush]);

  useEffect(() => {
    return () => {
      if (timerRef.current) clearTimeout(timerRef.current);
    };
  }, []);

  return { saving, lastSaved, saveNow };
}
