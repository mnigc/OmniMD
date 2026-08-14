export interface ConversionTask {
  id: string;
  sourcePath: string;
  outputPath: string;
  status: TaskStatus;
  progress: number;
  stage: ConversionStage;
  error: string | null;
  createdAt: number;
  completedAt: number | null;
}

export interface ConversionResult {
  taskId: string;
  markdown: string;
  documentSerialized: string;
  assetCount: number;
  errors: ErrorDto[];
  success: boolean;
}

export interface ErrorDto {
  code: string;
  message: string;
  retryable: boolean;
}

export interface BatchResultDto {
  total: number;
  completed: number;
  failed: number;
  results: ConversionResult[];
}

export interface TaskProgressDto {
  taskId: string;
  progress: number;
  stage: string;
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
  | "DetectingFormat"
  | "Extracting"
  | "Ocr"
  | "Structuring"
  | "Serializing"
  | "Saving";
