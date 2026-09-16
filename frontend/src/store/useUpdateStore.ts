import { create } from "zustand";
import {
  checkForUpdate,
  installPendingUpdate,
  relaunchApp,
  type DownloadEvent,
} from "../api/updater";

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
            : { status: "up-to-date", version: null, notes: null, date: null }
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
      });
    } catch (err) {
      // Offline, rate-limited, or a dev build without updater config: a
      // background check must never nag the user about it.
      set(
        silent
          ? { status: "idle" }
          : {
              status: "error",
              error: err instanceof Error ? err.message : String(err),
            }
      );
    }
  },

  install: async () => {
    if (get().status !== "available" && get().status !== "error") return;
    set({ status: "downloading", progress: 0, error: null });

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
      });
    }
  },

  restart: async () => {
    await relaunchApp();
  },

  openDialog: () => set({ dialogOpen: true }),
  closeDialog: () => {
    const status = get().status;
    // Never let the user dismiss the dialog while an install is in flight.
    if (status === "downloading" || status === "installing") return;
    set({ dialogOpen: false });
  },
}));
