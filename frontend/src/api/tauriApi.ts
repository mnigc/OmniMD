import { invoke } from "@tauri-apps/api/core";
import type {
  ConversionResult,
  WorkspaceInfo,
  LibraryDocument,
  LibraryFolder,
  SearchHit,
  ScanResult,
  BatchTaskDto,
  BatchSummaryDto,
} from "../types";

export async function convertFile(
  sourcePath: string,
  outputDir: string,
  clientTaskId?: string
): Promise<ConversionResult> {
  return invoke<ConversionResult>("convert_file", {
    sourcePath,
    outputDir,
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

export async function getAppVersion(): Promise<string> {
  return invoke<string>("get_app_version");
}

export async function writeTextFile(
  path: string,
  content: string
): Promise<void> {
  return invoke<void>("write_text_file", { path, content });
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
  outputPath: string
): Promise<string> {
  return invoke<string>("batch_enqueue", {
    sourcePath,
    outputPath,
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

// ---- Settings: storage (cache & logs) ----

export interface CacheInfo {
  /** WebView2 磁盘缓存字节数；非 Windows 平台为 null（不支持清理）。 */
  webviewCacheBytes: number | null;
  logsBytes: number;
}

export async function getCacheInfo(): Promise<CacheInfo> {
  return invoke<CacheInfo>("get_cache_info");
}

/** 清除 WebView2 磁盘缓存（不动 localStorage/设置），返回清理后的最新占用。 */
export async function clearWebviewCache(): Promise<CacheInfo> {
  return invoke<CacheInfo>("clear_webview_cache");
}

/** 清理应用日志（当前日志截断、轮转副本删除），返回清理后的最新占用。 */
export async function clearLogs(): Promise<CacheInfo> {
  return invoke<CacheInfo>("clear_logs");
}
