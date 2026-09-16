import { create } from "zustand";
import {
  checkForUpdate,
  installPendingUpdate,
  relaunchApp,
  type DownloadEvent,
} from "../api/updater";
import { translate } from "../i18n";

export type UpdateStatus =
  | "idle"
  | "checking"
  | "up-to-date"
  | "available"
  | "downloading"
  | "installing"
  | "ready"
  | "error";

interface UpdateState {
  status: UpdateStatus;
  version: string | null;
  currentVersion: string | null;
  notes: string | null;
  date: string | null;
  progress: number | null;
  error: string | null;
  /** 错误来源：决定 Retry 按钮应重新检查还是重新安装。 */
  errorPhase: "check" | "install" | null;
  dialogOpen: boolean;

  check: (options?: { silent?: boolean }) => Promise<void>;
  install: () => Promise<void>;
  restart: () => Promise<void>;
  openDialog: () => void;
  closeDialog: () => void;
}

export const useUpdateStore = create<UpdateState>((set, get) => ({
  status: "idle",
  version: null,
  currentVersion: null,
  notes: null,
  date: null,
  progress: null,
  error: null,
  errorPhase: null,
  dialogOpen: false,

  /**
   * `silent` is used for the background check on startup: failures and the
   * "already up to date" result must not surface any UI.
   */
  check: async ({ silent = false } = {}) => {
    const current = get().status;
    if (current === "checking" || current === "downloading" || current === "installing") {
      return;
    }
    if (!silent) set({ status: "checking", error: null });

    try {
      const info = await checkForUpdate();
      if (!info) {
        set(
          silent
            ? { status: "idle" }
            : { status: "up-to-date", version: null, notes: null, date: null, errorPhase: null }
        );
        return;
      }
      set({
        status: "available",
        version: info.version,
        currentVersion: info.currentVersion,
        notes: info.body ?? null,
        date: info.date ?? null,
        error: null,
        errorPhase: null,
      });
    } catch (err) {
      // Offline, rate-limited, or a dev build without updater config: a
      // background check must never nag the user about it — and must never
      // clobber an "available" result that a previous successful check found.
      if (silent) {
        if (get().status !== "available") set({ status: "idle" });
        return;
      }
      set({
        status: "error",
        error: err instanceof Error ? err.message : String(err),
        errorPhase: "check",
      });
    }
  },

  install: async () => {
    const state = get();
    if (state.status !== "available" && state.status !== "error") return;
    // check 阶段的失败（断网等）意味着更新句柄已被释放，Retry 走安装
    // 只会再次报 "No pending update"——必须重新执行 check。
    if (state.status === "error" && state.errorPhase === "check") {
      await get().check();
      return;
    }
    set({ status: "downloading", progress: 0, error: null, errorPhase: null });

    let downloaded = 0;
    let total: number | undefined;
    const onEvent = (event: DownloadEvent) => {
      switch (event.event) {
        case "Started":
          total = event.data.contentLength;
          downloaded = 0;
          set({ status: "downloading", progress: total ? 0 : null });
          break;
        case "Progress":
          downloaded += event.data.chunkLength;
          set({
            progress: total ? Math.round((downloaded / total) * 100) : null,
          });
          break;
        case "Finished":
          set({ status: "installing", progress: 100 });
          break;
      }
    };

    try {
      await installPendingUpdate(onEvent);
      // Unreachable on Windows (the installer exits the app first).
      set({ status: "ready" });
    } catch (err) {
      set({
        status: "error",
        progress: null,
        error: err instanceof Error ? err.message : String(err),
        errorPhase: "install",
      });
    }
  },

  restart: async () => {
    try {
      await relaunchApp();
    } catch (err) {
      // relaunch 失败（权限/环境问题）时按钮此前会静默无效：给出可见
      // 错误，让用户知道需要手动重启。
      set({
        status: "error",
        error: `${translate("update.relaunchFailed")}: ${err instanceof Error ? err.message : String(err)}`,
        errorPhase: "install",
      });
    }
  },

  openDialog: () => set({ dialogOpen: true }),
  closeDialog: () => {
    const status = get().status;
    // Never let the user dismiss the dialog while an install is in flight.
    if (status === "downloading" || status === "installing") return;
    set({ dialogOpen: false });
  },
}));
