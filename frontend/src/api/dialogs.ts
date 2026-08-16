import { open } from "@tauri-apps/plugin-dialog";

export async function pickFiles(formats: string[]): Promise<string[]> {
  const selected = await open({
    multiple: true,
    directory: false,
    title: "Select files to convert",
    filters:
      formats.length > 0
        ? [{ name: "Documents", extensions: formats }]
        : undefined,
  });
  if (selected === null) return [];
  return Array.isArray(selected) ? selected : [selected];
}

export async function pickDir(): Promise<string | null> {
  return open({
    directory: true,
    multiple: false,
    title: "Select folder to convert",
  });
}

export async function pickOutputDir(): Promise<string | null> {
  return open({
    directory: true,
    multiple: false,
    title: "Select output folder",
  });
}
