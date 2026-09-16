import { confirm, open } from "@tauri-apps/plugin-dialog";

export async function pickFiles(
  formats: string[],
  title = "Select files to convert"
): Promise<string[]> {
  const selected = await open({
    multiple: true,
    directory: false,
    title,
    filters:
      formats.length > 0
        ? [{ name: "Documents", extensions: formats }]
        : undefined,
  });
  if (selected === null) return [];
  return Array.isArray(selected) ? selected : [selected];
}

export async function pickDir(title = "Select folder"): Promise<string | null> {
  return open({
    directory: true,
    multiple: false,
    title,
  });
}

export async function pickOutputDir(
  title = "Select output folder"
): Promise<string | null> {
  return open({
    directory: true,
    multiple: false,
    title,
  });
}

/**
 * Native confirmation dialog, shared by every "destructive" action so the
 * styling is consistent (and the main thread is never blocked by `window.confirm`).
 */
export async function confirmDialog(
  message: string,
  title?: string,
  kind: "info" | "warning" | "error" = "warning"
): Promise<boolean> {
  return confirm(message, { title, kind });
}
