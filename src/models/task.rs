use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::SystemTime;

use super::document::{Asset, Document};

/// Optional progress callback: receives a value in [0.0, 1.0] and an optional
/// human-readable detail string (e.g. the current conversion stage).
/// Wrapped in `Arc` so it can be shared/cloned across threads.
pub type ProgressCallback = Arc<dyn Fn(f32, Option<String>) + Send + Sync>;

/// Shared cooperative-cancellation flag. `cancelled()` becomes `true` once
/// any caller requests the running conversion to stop.
#[derive(Debug, Clone, Default)]
pub struct Cancellation(Arc<std::sync::atomic::AtomicBool>);

impl Cancellation {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        self.0.store(true, std::sync::atomic::Ordering::Relaxed);
    }

    pub fn cancelled(&self) -> bool {
        self.0.load(std::sync::atomic::Ordering::Relaxed)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TaskStatus {
    Pending,
    Processing,
    Completed,
    Failed,
    Cancelled,
}

/// Task-level progress stages surfaced to the UI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConversionStage {
    Queued,
    Fetching,
    ModelLoading,
    Parsing,
    PostProcessing,
    Saving,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ErrorCode {
    Unsupported,
    Encrypted,
    Malformed,
    EngineError,
    RuntimeNotReady,
    IoError,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversionError {
    pub code: ErrorCode,
    pub message: String,
    pub stage: ConversionStage,
    pub retryable: bool,
    /// 1-based page number this error originated from (for per-page failures).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page: Option<u32>,
}

/// Statistics about a single conversion, computed by the pipeline.
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
    /// short-circuited before the pipeline could measure anything).
    #[serde(default)]
    pub stats: Option<ConversionStats>,
}

// ---------------------------------------------------------------------------
// Batch task DTOs
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchTaskDto {
    pub id: String,
    pub source_path: String,
    pub output_path: String,
    pub status: String,
    pub progress: f32,
    pub stage: String,
    pub error: Option<String>,
    pub created_at: u64,
    pub completed_at: Option<u64>,
    pub elapsed_secs: u64,
    pub retry_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchSummaryDto {
    pub total: u64,
    pub pending: u64,
    pub processing: u64,
    pub completed: u64,
    pub failed: u64,
    pub cancelled: u64,
    pub paused: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchFilter {
    pub status: Option<String>,
    pub limit: Option<u64>,
    pub offset: Option<u64>,
}

impl ConversionTask {
    pub fn new(source_path: &str, output_path: &str) -> Self {
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
            stage: ConversionStage::Queued,
            error: None,
            created_at: now,
            completed_at: None,
        }
    }
}
