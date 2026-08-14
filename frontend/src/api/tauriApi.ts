import { invoke } from "@tauri-apps/api/core";
import type {
  ConversionResult,
  BatchResultDto,
  ConverterInfo,
} from "../types";

export async function convertFile(
  sourcePath: string,
  outputDir: string
): Promise<ConversionResult> {
  return invoke<ConversionResult>("convert_file", {
    sourcePath,
    outputDir,
  });
}

export async function convertBatch(
  sourcePaths: string[],
  outputDir: string,
  concurrency: number
): Promise<BatchResultDto> {
  return invoke<BatchResultDto>("convert_batch", {
    sourcePaths,
    outputDir,
    concurrency,
  });
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
