import {
  check,
  type Update,
  type DownloadEvent,
} from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";

export type { DownloadEvent };

export interface UpdateInfo {
  version: string;
  currentVersion: string;
  date?: string;
  body?: string;
}

/**
 * The updater returns a `Resource` handle that must stay alive between the
 * check and the install step, so it is kept in module scope instead of React
 * state (which would be recreated on every render).
 */
let pending: Update | null = null;

/** Ask the configured endpoint whether a newer release exists. */
export async function checkForUpdate(): Promise<UpdateInfo | null> {
  await releasePending();
  const update = await check({ timeout: 30_000 });
  if (!update) return null;

  pending = update;
  return {
    version: update.version,
    currentVersion: update.currentVersion,
    date: update.date,
    body: update.body,
  };
}

/**
 * Download and install the update found by {@link checkForUpdate}.
 *
 * On Windows the app is exited by the installer; on macOS/Linux the caller has
 * to relaunch the app for the new version to take effect.
 */
export async function installPendingUpdate(
  onEvent: (event: DownloadEvent) => void
): Promise<void> {
  const update = pending;
  if (!update) throw new Error("No pending update");
  await update.downloadAndInstall(onEvent);
  // Only release on success: a failed attempt keeps the handle so the user can
  // retry without running the whole check again.
  await releasePending();
}

export async function relaunchApp(): Promise<void> {
  await relaunch();
}

async function releasePending(): Promise<void> {
  const update = pending;
  pending = null;
  if (!update) return;
  try {
    await update.close();
  } catch {
    // Already released by the backend (e.g. after an install) — nothing to do.
  }
}
