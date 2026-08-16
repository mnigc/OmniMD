export interface ConversionTask {
  id: string;
  sourcePath: string;
  outputPath: string;
  outputDir: string;
  outputMode: OutputMode;
  ocrMode?: "off" | "auto" | "always";
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
  ocrPageCount?: number;
  ocrCharCount?: number;
  avgConfidencePermille?: number;
  lowConfidenceCount?: number;
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
  | "Fetching"
  | "DetectingFormat"
  | "Extracting"
  | "Ocr"
  | "Structuring"
  | "Serializing"
  | "Saving";

export type OutputMode = "standard" | "aiReady" | "obsidian";