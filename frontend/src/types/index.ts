export interface ConversionTask {
  id: string;
  sourcePath: string;
  outputPath: string;
  outputDir: string;
  outputMode: OutputMode;
  parseQuality?: ParseQuality;
  status: TaskStatus;
  progress: number;
  stage: ConversionStage;
  error: string | null;
  createdAt: number;
  completedAt: number | null;
}

export interface HistoryEntry {
  id: string;
  sourcePath: string;
  outputPath: string;
  outputMode: OutputMode;
  status: TaskStatus;
  error: string | null;
  createdAt: number;
  completedAt: number | null;
}

export interface ConversionStats {
  imageCount: number;
  tableCount: number;
  wordCount: number;
}

export interface ConversionResult {
  taskId: string;
  markdown: string;
  documentSerialized: string;
  assetCount: number;
  errors: ErrorDto[];
  success: boolean;
  outputPath: string;
  stats: ConversionStats;
}

export interface ErrorDto {
  code: string;
  message: string;
  retryable: boolean;
}

export interface AiReadyOpts {
  genToc: boolean;
  genMeta: boolean;
}

export interface TaskProgressDto {
  taskId: string;
  progress: number;
  stage: string;
  detail?: string;
}

export interface TaskStatusDto {
  taskId: string;
  status: string;
  error: string | null;
}

export interface ConverterInfo {
  name: string;
  supportedFormats: string[];
}

export type TaskStatus =
  | "Pending"
  | "Processing"
  | "Completed"
  | "Failed"
  | "Cancelled";

export type ConversionStage =
  | "Queued"
  | "Fetching"
  | "ModelLoading"
  | "Parsing"
  | "PostProcessing"
  | "Saving";

export type OutputMode = "standard" | "aiReady" | "obsidian";

export type ParseQuality = "auto" | "quick" | "high";

// ---- M2 Workbench data layer ----

export interface WorkspaceInfo {
  id: number;
  name: string;
  path: string;
  createdAt: string;
  lastOpenedAt: string | null;
}

export interface LibraryDocument {
  id: number;
  workspaceId: number;
  path: string;
  title: string;
  fileSize: number;
  favorite: boolean;
  source: string | null;
  createdAt: string;
  openedAt: string | null;
}

export interface LibraryFolder {
  name: string;
  path: string;
  docCount: number;
}

export interface SearchHit {
  document: LibraryDocument;
  snippet: string | null;
}

export interface ScanResult {
  indexed: number;
  updated: number;
  removed: number;
  total: number;
}

// ---- Batch task types ----

export interface BatchTaskDto {
  id: string;
  sourcePath: string;
  outputPath: string;
  status: string;
  progress: number;
  stage: string;
  error: string | null;
  createdAt: number;
  completedAt: number | null;
  elapsedSecs: number;
  outputMode: OutputMode;
  parseQuality: ParseQuality;
  retryCount: number;
}

export interface BatchSummaryDto {
  total: number;
  pending: number;
  processing: number;
  completed: number;
  failed: number;
  cancelled: number;
  paused: number;
}

export interface BatchProgressEvent {
  taskId: string;
  progress: number;
  stage: string;
  elapsedSecs: number;
  detail?: string;
}

export interface BatchStatusEvent {
  taskId: string;
  status: string;
  error: string | null;
  elapsedSecs: number;
}

export interface BatchSummaryEvent {
  summary: BatchSummaryDto;
}

// ---- Model management types ----

export interface HardwareRequirements {
  minRamGb: number;
  recRamGb: number;
  gpuRequired: boolean;
  gpuVramGb: number;
  cpuOnlySupported: boolean;
  notes: string;
}

export interface ModelInfo {
  name: string;
  displayName: string;
  sizeBytes: number;
  status: string;
  path: string | null;
  downloadUrl: string | null;
  version: string | null;
  hardwareRequirements: HardwareRequirements;
}

export interface CacheInfo {
  path: string;
  totalSizeBytes: number;
}

export interface DownloadProgress {
  modelName: string;
  progress: number;
  speed: string;
  stage: string;
}