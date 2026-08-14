use serde::{Deserialize, Serialize};
use std::time::SystemTime;

use super::document::{Asset, Document};

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
    DetectingFormat,
    Extracting,
    Ocr,
    Structuring,
    Serializing,
    Saving,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ErrorCode {
    Unsupported,
    Encrypted,
    Malformed,
    OcrFailed,
    IoError,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversionError {
    pub code: ErrorCode,
    pub message: String,
    pub stage: ConversionStage,
    pub retryable: bool,
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
            stage: ConversionStage::DetectingFormat,
            error: None,
            created_at: now,
            completed_at: None,
        }
    }
}
