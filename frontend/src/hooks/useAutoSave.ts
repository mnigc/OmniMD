import { useCallback, useEffect, useRef, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { writeTextFile } from "../api/tauriApi";
import { showToast } from "../lib/toast";
import { useI18n } from "../i18n";

/** 写盘失败后的自动重试参数：3 秒一次，最多 3 次。 */
const RETRY_DELAY_MS = 3000;
const MAX_RETRIES = 3;

/**
 * Debounced autosave for a single edited file.
 *
 * Correctness guarantees:
 * - Pending edits are flushed to their ORIGINAL path before `filePath` changes,
 *   so switching documents never silently drops the previous file's changes.
 * - The saved baseline is per-file: content is only treated as an edit of the
 *   newly selected file once it DIFFERS from the content that was on screen at
 *   the moment of the switch. A slow/failed async load therefore can never
 *   smuggle the previous document's text into the new file.
 * - Pending edits are flushed on unmount, and window close is held until the
 *   pending write lands, so neither closing the window nor switching pages
 *   within the debounce window loses data.
 * - A failed write keeps the pending edit and retries automatically.
 */
export function useAutoSave(
  content: string,
  filePath: string | null,
  delay = 1500,
): {
  saving: boolean;
  lastSaved: number | null;
  dirty: boolean;
  saveNow: (contentOverride?: string) => Promise<void>;
} {
  const { t } = useI18n();
  const [saving, setSaving] = useState(false);
  const [lastSaved, setLastSaved] = useState<number | null>(null);
  const [dirty, setDirty] = useState(false);
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const filePathRef = useRef<string | null>(filePath);
  const baselineRef = useRef(content);
  const pendingRef = useRef<{ path: string; content: string } | null>(null);
  const awaitingLoadRef = useRef(true);
  // filePath 切换瞬间屏幕上显示的内容：异步加载完成前到达的 content effect
  // 若仍等于该值，说明新文件内容尚未真正加载，绝不能当作新文件的 baseline。
  const switchContentRef = useRef(content);
  const retriesRef = useRef(0);
  const mountedRef = useRef(true);

  const schedule = useCallback(
    (ms: number) => {
      if (timerRef.current) clearTimeout(timerRef.current);
      timerRef.current = setTimeout(() => {
        timerRef.current = null;
        void flushRef.current();
      }, ms);
    },
    []
  );

  const flush = useCallback(async () => {
    if (timerRef.current) {
      clearTimeout(timerRef.current);
      timerRef.current = null;
    }
    const pending = pendingRef.current;
    if (!pending) return;
    pendingRef.current = null;

    if (mountedRef.current) setSaving(true);
    try {
      await writeTextFile(pending.path, pending.content);
      retriesRef.current = 0;
      if (pending.path === filePathRef.current) {
        baselineRef.current = pending.content;
        if (mountedRef.current) {
          setLastSaved(Date.now());
          setDirty(false);
        }
      }
    } catch (err) {
      // Keep the pending edit and retry a few times automatically; a user
      // who stops typing right after a failure must not lose the edit.
      pendingRef.current = pending;
      if (retriesRef.current < MAX_RETRIES) {
        retriesRef.current += 1;
        schedule(RETRY_DELAY_MS);
        return;
      }
      showToast(
        t("editor.saveFailed", {
          error: err instanceof Error ? err.message : String(err),
        }),
        3000,
        "error"
      );
    } finally {
      if (mountedRef.current) setSaving(false);
    }
  }, [t, schedule]);

  // flush is used from timers registered inside itself; keep a ref so the
  // timer callback always invokes the latest closure without re-scheduling.
  const flushRef = useRef(flush);
  flushRef.current = flush;

  const saveNow = useCallback(
    (contentOverride?: string) => {
      // 新文件尚未真正加载完成时，编辑器里还是上一个文档的内容，此时
      // 不能把它当编辑写入新路径——只 flush 已排队的旧文件待写项。
      if (
        contentOverride !== undefined &&
        !awaitingLoadRef.current &&
        filePathRef.current &&
        contentOverride !== baselineRef.current
      ) {
        pendingRef.current = { path: filePathRef.current, content: contentOverride };
        setDirty(true);
      }
      return flush();
    },
    [flush]
  );

  // Runs BEFORE the scheduling effect on a file switch (declaration order).
  useEffect(() => {
    const prev = filePathRef.current;
    if (prev === filePath) return;
    // Flush edits that belong to the file we are leaving.
    if (pendingRef.current && pendingRef.current.path === prev) {
      void flushRef.current();
    }
    pendingRef.current = null;
    retriesRef.current = 0;
    filePathRef.current = filePath;
    switchContentRef.current = content;
    awaitingLoadRef.current = true;
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [filePath]);

  useEffect(() => {
    if (!filePath) return;
    if (awaitingLoadRef.current) {
      if (content === switchContentRef.current) {
        // 新文件的真实内容还没加载进来（仍是切换瞬间的旧内容）：
        // 不设 baseline、不调度保存，等真正的加载结果。
        return;
      }
      // First genuinely-new content for this file is its baseline, not an edit.
      baselineRef.current = content;
      awaitingLoadRef.current = false;
      setDirty(false);
      return;
    }
    if (content === baselineRef.current) return;
    pendingRef.current = { path: filePath, content };
    setDirty(true);
    schedule(delay);
    return () => {
      if (timerRef.current) clearTimeout(timerRef.current);
      timerRef.current = null;
    };
  }, [content, filePath, delay, schedule]);

  // 卸载时把尚未落盘的编辑立即写出去（fire-and-forget；卸载后不能再
  // setState，写结果被有意忽略）。
  useEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
      if (timerRef.current) {
        clearTimeout(timerRef.current);
        timerRef.current = null;
      }
      const pending = pendingRef.current;
      pendingRef.current = null;
      if (pending) {
        void writeTextFile(pending.path, pending.content).catch(() => {});
      }
    };
  }, []);

  // 拦截窗口关闭：先落盘再放行，关窗瞬间不再丢 1.5 秒防抖窗口内的编辑。
  // 写盘失败时不销毁窗口——用户能看到错误提示并再次尝试关闭。
  useEffect(() => {
    let cleanedUp = false;
    let unlisten: (() => void) | undefined;
    const promise = getCurrentWindow().onCloseRequested(async (event) => {
      const pending = pendingRef.current;
      if (!pending) return;
      event.preventDefault();
      try {
        await writeTextFile(pending.path, pending.content);
        pendingRef.current = null;
        retriesRef.current = 0;
        await getCurrentWindow().destroy();
      } catch (err) {
        pendingRef.current = pending;
        showToast(
          t("editor.saveFailed", {
            error: err instanceof Error ? err.message : String(err),
          }),
          3000,
          "error"
        );
      }
    });
    promise
      .then((fn) => {
        if (cleanedUp) fn();
        else unlisten = fn;
      })
      .catch(() => {});
    return () => {
      cleanedUp = true;
      unlisten?.();
    };
  }, [t]);

  return { saving, lastSaved, dirty, saveNow };
}
