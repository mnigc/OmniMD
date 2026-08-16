import { invoke } from "@tauri-apps/api/core";
import type {
  ConversionResult,
  ConverterInfo,
  OutputMode,
  AiReadyOpts,
} from "../types";
import type { OcrMode } from "../store/useSettingsStore";

export async function convertFile(
  sourcePath: string,
  outputDir: string,
  outputMode?: OutputMode,
  aiReadyOpts?: AiReadyOpts,
  ocrMode?: OcrMode,
  clientTaskId?: string
): Promise<ConversionResult> {
  return invoke<ConversionResult>("convert_file", {
    sourcePath,
    outputDir,
    outputMode: outputMode ?? null,
    aiReadyOpts: aiReadyOpts ?? null,
    ocrMode: ocrMode ?? null,
    clientTaskId: clientTaskId ?? null,
  });
}

/** Ask the backend to stop a running conversion at the next checkpoint. */
export async function cancelTask(taskId: string): Promise<void> {
  return invoke<void>("cancel_task", { taskId });
}

export async function getSupportedFormats(): Promise<string[]> {
  return invoke<string[]>("get_supported_formats");
}

export async function getConverterInfo(): Promise<ConverterInfo> {
  const raw = await invoke<string>("get_converter_info");
  return JSON.parse(raw);
}

export async function previewMarkdown(markdown: string): Promise<string> {
  return invoke<string>("preview_markdown", { markdown });
}

export async function writeTextFile(
  path: string,
  content: string
): Promise<void> {
  return invoke<void>("write_text_file", { path, content });
}

export async function fetchUrl(
  url: string,
  outputDir: string,
  outputMode?: OutputMode,
  aiReadyOpts?: AiReadyOpts,
  clientTaskId?: string
): Promise<ConversionResult> {
  return invoke<ConversionResult>("fetch_url", {
    url,
    outputDir,
    outputMode: outputMode ?? null,
    aiReadyOpts: aiReadyOpts ?? null,
    clientTaskId: clientTaskId ?? null,
  });
}

export async function downloadUrl(url: string): Promise<string> {
  return invoke<string>("download_url", { url });
}

export async function readTextFile(path: string): Promise<string> {
  return invoke<string>("read_text_file", { path });
}

export async function listFilesInFolder(path: string): Promise<string[]> {
  return invoke<string[]>("list_files_in_folder", { path });
}

export async function getDefaultOutputDir(): Promise<string> {
  return invoke<string>("get_default_output_dir");
}

export async function openFolder(path: string): Promise<void> {
  return invoke<void>("open_folder", { path });
}