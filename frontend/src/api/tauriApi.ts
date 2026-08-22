import { invoke } from "@tauri-apps/api/core";
import type {
  ConversionResult,
  ConverterInfo,
  OutputMode,
  AiReadyOpts,
  ParseQuality,
  WorkspaceInfo,
  LibraryDocument,
  LibraryFolder,
  SearchHit,
  ScanResult,
  BatchTaskDto,
  BatchSummaryDto,
  ModelInfo,
  CacheInfo,
} from "../types";

export async function convertFile(
  sourcePath: string,
  outputDir: string,
  outputMode?: OutputMode,
  aiReadyOpts?: AiReadyOpts,
  parseQuality?: ParseQuality,
  clientTaskId?: string
): Promise<ConversionResult> {
  return invoke<ConversionResult>("convert_file", {
    sourcePath,
    outputDir,
    outputMode: outputMode ?? null,
    aiReadyOpts: aiReadyOpts ?? null,
    parseQuality: parseQuality ?? null,
    clientTaskId: clientTaskId ?? null,
  });
}

/** Ensure the bundled MinerU API service subprocess is running. */
export async function startMineru(): Promise<string> {
  return invoke<string>("start_mineru");
}

export interface MineruStatus {
  healthy: boolean;
  baseUrl: string;
}

/** Probe the status of the MinerU API service subprocess. */
export async function mineruStatus(): Promise<MineruStatus> {
  return invoke<MineruStatus>("mineru_status");
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

export async function getAppVersion(): Promise<string> {
  return invoke<string>("get_app_version");
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

// ---- M2 Workbench data layer ----

export async function listWorkspaces(): Promise<WorkspaceInfo[]> {
  return invoke<WorkspaceInfo[]>("list_workspaces");
}

export async function addWorkspace(
  name: string,
  path: string
): Promise<WorkspaceInfo> {
  return invoke<WorkspaceInfo>("add_workspace", { name, path });
}

export async function removeWorkspace(id: number): Promise<void> {
  return invoke<void>("remove_workspace", { id });
}

export async function getActiveWorkspace(): Promise<WorkspaceInfo | null> {
  return invoke<WorkspaceInfo | null>("get_active_workspace");
}

export async function setActiveWorkspace(id: number): Promise<void> {
  return invoke<void>("set_active_workspace", { id });
}

/** Incrementally re-index every .md file under the workspace root. */
export async function scanWorkspace(id: number): Promise<ScanResult> {
  return invoke<ScanResult>("scan_workspace", { id });
}

export async function listDocuments(
  workspaceId: number,
  folder?: string
): Promise<LibraryDocument[]> {
  return invoke<LibraryDocument[]>("list_documents", {
    workspaceId,
    folder: folder ?? null,
  });
}

export async function listSubfolders(
  workspaceId: number,
  folder?: string
): Promise<LibraryFolder[]> {
  return invoke<LibraryFolder[]>("list_subfolders", {
    workspaceId,
    folder: folder ?? null,
  });
}

export async function listFavorites(workspaceId: number): Promise<LibraryDocument[]> {
  return invoke<LibraryDocument[]>("list_favorites", { workspaceId });
}

export async function listRecent(workspaceId?: number, limit?: number): Promise<LibraryDocument[]> {
  return invoke<LibraryDocument[]>("list_recent", { workspaceId: workspaceId ?? null, limit: limit ?? 20 });
}

export async function setDocumentFavorite(
  id: number,
  favorite: boolean
): Promise<void> {
  return invoke<void>("set_document_favorite", { id, favorite });
}

export async function recordDocumentOpen(id: number): Promise<void> {
  return invoke<void>("record_document_open", { id });
}

export async function searchDocuments(
  query: string,
  workspaceId: number,
  limit?: number
): Promise<SearchHit[]> {
  return invoke<SearchHit[]>("search_documents", {
    query,
    workspaceId,
    limit: limit ?? 50,
  });
}

// ---- Batch task API ----

export async function batchEnqueue(
  sourcePath: string,
  outputPath: string,
  outputMode?: OutputMode,
  parseQuality?: ParseQuality
): Promise<string> {
  return invoke<string>("batch_enqueue", {
    sourcePath,
    outputPath,
    outputMode: outputMode ?? null,
    parseQuality: parseQuality ?? null,
  });
}

export async function batchStart(): Promise<void> {
  return invoke<void>("batch_start");
}

export async function batchPauseTask(taskId: string): Promise<void> {
  return invoke<void>("batch_pause_task", { taskId });
}

export async function batchResumeTask(taskId: string): Promise<void> {
  return invoke<void>("batch_resume_task", { taskId });
}

export async function batchCancelTask(taskId: string): Promise<void> {
  return invoke<void>("batch_cancel_task", { taskId });
}

export async function batchCancelAll(): Promise<void> {
  return invoke<void>("batch_cancel_all");
}

export async function batchRetryFailed(): Promise<void> {
  return invoke<void>("batch_retry_failed");
}

export async function batchRetryTask(taskId: string): Promise<void> {
  return invoke<void>("batch_retry_task", { taskId });
}

export async function batchClearDone(): Promise<void> {
  return invoke<void>("batch_clear_done");
}

export async function batchSetConcurrency(concurrency: number): Promise<void> {
  return invoke<void>("batch_set_concurrency", { concurrency });
}

export async function batchListTasks(): Promise<BatchTaskDto[]> {
  return invoke<BatchTaskDto[]>("batch_list_tasks");
}

export async function batchGetSummary(): Promise<BatchSummaryDto> {
  return invoke<BatchSummaryDto>("batch_get_summary");
}

// ---- Model management API ----

export async function listModels(): Promise<ModelInfo[]> {
  return invoke<ModelInfo[]>("list_models");
}

export async function getModelStatus(modelName: string): Promise<ModelInfo> {
  return invoke<ModelInfo>("get_model_status", { modelName });
}

export async function downloadModel(modelName: string): Promise<void> {
  return invoke<void>("download_model", { modelName });
}

export async function cancelModelDownload(): Promise<void> {
  return invoke<void>("cancel_model_download");
}

export async function getCacheInfo(): Promise<CacheInfo> {
  return invoke<CacheInfo>("get_cache_info");
}

export async function clearModelCache(): Promise<void> {
  return invoke<void>("clear_model_cache");
}

export async function setModelSource(source: string): Promise<void> {
  return invoke<void>("set_model_source", { source });
}

export async function getModelSource(): Promise<string> {
  return invoke<string>("get_model_source");
}

export async function importOfflineModel(path: string): Promise<void> {
  return invoke<void>("import_offline_model", { path });
}

export async function checkModelUpdate(modelName: string): Promise<boolean> {
  return invoke<boolean>("check_model_update", { modelName });
}

export async function isModelDownloaded(): Promise<boolean> {
  return invoke<boolean>("is_model_downloaded");
}

export async function checkPythonEnvironment(): Promise<boolean> {
  return invoke<boolean>("check_python_environment");
}

export async function setupPythonEnvironment(): Promise<void> {
  return invoke<void>("setup_python_environment");
}

/** One-time, fully automatic environment preparation (Python + model + MinerU). */
export async function prepareEnvironment(): Promise<void> {
  return invoke<void>("prepare_environment");
}