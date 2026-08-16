use serde::{Deserialize, Serialize};
use std::time::SystemTime;

use super::document::{Asset, Document};
use super::ocr::OcrMode;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TaskStatus {
    Pending,
    Processing,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConversionStage {
    Fetching,
    DetectingFormat,
    Extracting,
    Ocr,
    Structuring,
    Serializing,
    Saving,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ErrorCode {
    Unsupported,
    Encrypted,
    Malformed,
    OcrFailed,
    IoError,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum OutputMode {
    Standard,
    AiReady,
    Obsidian,
}

impl Default for OutputMode {
    fn default() -> Self {
        OutputMode::AiReady
    }
}

impl OutputMode {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "standard" => OutputMode::Standard,
            "obsidian" => OutputMode::Obsidian,
            _ => OutputMode::AiReady,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversionError {
    pub code: ErrorCode,
    pub message: String,
    pub stage: ConversionStage,
    pub retryable: bool,
    /// 1-based page number this error originated from (for per-page OCR failures).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page: Option<u32>,
}

/// Statistics about a single conversion, computed by `pipeline::convert_file`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ConversionStats {
    /// Number of image references (`![..]( ... )`) in the markdown,
    /// which equals the number of bundled assets.
    pub image_count: usize,
    /// Number of markdown tables (separator rows `|---`) in the output.
    pub table_count: usize,
    /// Number of words in the output markdown.
    /// For CJK scripts counts characters, for Latin scripts counts words.
    pub word_count: usize,
    /// Number of pages processed by OCR (0 if OCR was not used).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ocr_page_count: Option<usize>,
    /// Total characters recognized by OCR.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ocr_char_count: Option<usize>,
    /// Average confidence per mille (0-1000) of OCR results.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub avg_confidence_permille: Option<u32>,
    /// Number of low-confidence blocks (< 0.5).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub low_confidence_count: Option<usize>,
}

/// Options controlling the AI Ready formatter (see `markdown_pipeline`).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct AiReadyOpts {
    /// Insert a generated table of contents at the top of the document.
    pub gen_toc: bool,
    /// Insert a controlled metadata comment block at the top of the document.
    pub gen_meta: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversionTask {
    pub id: String,
    pub source_path: String,
    pub output_path: String,
    pub status: TaskStatus,
    pub progress: f32,
    pub stage: ConversionStage,
    pub error: Option<String>,
    pub created_at: u64,
    pub completed_at: Option<u64>,
    #[serde(default)]
    pub output_mode: OutputMode,
    #[serde(default)]
    pub ai_ready_opts: AiReadyOpts,
    #[serde(default)]
    pub ocr_mode: OcrMode,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversionResult {
    pub task_id: String,
    pub markdown: String,
    pub document: Document,
    pub assets: Vec<Asset>,
    pub errors: Vec<ConversionError>,
    /// Final absolute path the markdown was written to. May be empty when the
    /// conversion failed before writing (e.g. batch error fallback).
    #[serde(default)]
    pub output_path: String,
    /// Conversion statistics. `None` when not yet computed (e.g. errors that
    /// short-circuited before `convert_file` could measure anything).
    #[serde(default)]
    pub stats: Option<ConversionStats>,
}

impl ConversionTask {
    pub fn new(source_path: &str, output_path: &str) -> Self {
        Self::with_mode(source_path, output_path, OutputMode::default())
    }

    pub fn with_mode(source_path: &str, output_path: &str, mode: OutputMode) -> Self {
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        ConversionTask {
            id: uuid::Uuid::new_v4().to_string(),
            source_path: source_path.to_string(),
            output_path: output_path.to_string(),
            status: TaskStatus::Pending,
            progress: 0.0,
            stage: ConversionStage::DetectingFormat,
            error: None,
            created_at: now,
            completed_at: None,
            output_mode: mode,
            ai_ready_opts: AiReadyOpts::default(),
            ocr_mode: OcrMode::default(),
        }
    }
}
