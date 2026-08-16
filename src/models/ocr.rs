use serde::{Deserialize, Serialize};
use std::sync::Arc;

use super::task::ConversionError;

/// Optional progress callback: receives a value in [0.0, 1.0] and an optional
/// human-readable detail string (e.g. current page of a multi-page OCR).
/// Wrapped in `Arc` so it can be shared/cloned across fallback paths.
pub type ProgressCallback = Arc<dyn Fn(f32, Option<String>) + Send + Sync>;

/// OCR mode: when to run OCR on a document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum OcrMode {
    Off,
    Auto,
    Always,
}

impl Default for OcrMode {
    fn default() -> Self {
        OcrMode::Auto
    }
}

/// A single recognized text block from OCR.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OcrBlock {
    /// Type of block (e.g. "text", "title", "table")
    #[serde(rename = "blockType")]
    pub block_type: String,
    /// Recognized text content
    pub text: String,
    /// Confidence score [0.0, 1.0]
    pub confidence: f32,
    /// Bounding box as [x_min, y_min, x_max, y_max] in page coordinates
    pub bbox: [f32; 4],
    /// 1-based page number
    pub page: u32,
    /// Reading order on the page
    pub order: u32,
}

/// OCR result for a single page.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OcrResult {
    /// 1-based page number
    pub page: u32,
    /// Page width in pixels
    pub width: u32,
    /// Page height in pixels
    pub height: u32,
    /// Recognized text blocks, ordered by reading order
    pub blocks: Vec<OcrBlock>,
}

/// Configuration for OCR engine behavior.
#[derive(Debug, Clone)]
pub struct OcrConfig {
    /// OCR mode: off/auto/always
    pub mode: OcrMode,
    /// Language(s) for recognition (e.g. "auto", "zh", "en", "zh+en")
    pub language: String,
    /// Optional custom model path override
    pub model_path: Option<String>,
    /// Runtime backend identifier
    pub runtime: String,
    /// Maximum time in seconds for a single OCR invocation before timing out.
    pub timeout_secs: u64,
}

impl Default for OcrConfig {
    fn default() -> Self {
        OcrConfig {
            mode: OcrMode::Auto,
            language: "zh+en".to_string(),
            model_path: None,
            runtime: "ort".to_string(),
            timeout_secs: 120,
        }
    }
}

/// OCR engine trait.
pub trait OcrEngine: Send + Sync {
    fn name(&self) -> &str;
    fn is_available(&self) -> bool;

    /// Perform OCR on the given image bytes with config, progress, and cancellation.
    fn recognize_image(
        &self,
        image: &[u8],
        config: &OcrConfig,
        on_progress: Option<ProgressCallback>,
        cancelled: Option<&Cancellation>,
    ) -> Result<OcrResult, ConversionError>;
}

/// Shared cooperative-cancellation flag. `cancelled()` becomes `true` once
/// any caller requests the running conversion to stop.
#[derive(Debug, Clone, Default)]
pub struct Cancellation(std::sync::Arc<std::sync::atomic::AtomicBool>);

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