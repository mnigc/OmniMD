use async_trait::async_trait;

use crate::models::ocr::{Cancellation, ProgressCallback};
use crate::models::task::{ConversionError, ConversionResult, ConversionStage, ConversionTask, ErrorCode};
use crate::engine::DocumentEngine;

/// Placeholder engine used after the bundled recognition engine was removed.
///
/// It reports that no document-to-Markdown engine is currently integrated so
/// the app still builds and runs; a real engine can be plugged in later by
/// implementing [`DocumentEngine`] and returning it from `AppState::create_engine`.
pub struct StubEngine;

impl StubEngine {
    pub fn new() -> Self {
        StubEngine
    }
}

impl Default for StubEngine {
    fn default() -> Self {
        StubEngine::new()
    }
}

#[async_trait]
impl DocumentEngine for StubEngine {
    fn name(&self) -> &str {
        "none"
    }

    fn is_available(&self) -> bool {
        false
    }

    async fn convert(
        &self,
        _task: &ConversionTask,
        _on_progress: Option<ProgressCallback>,
        _cancelled: Option<&Cancellation>,
    ) -> Result<ConversionResult, ConversionError> {
        Err(ConversionError {
            code: ErrorCode::EngineError,
            message: "未集成任何文档解析引擎（MinerU 已移除）。请在设置中接入其他识别引擎后再试。".to_string(),
            stage: ConversionStage::Parsing,
            retryable: false,
            page: None,
        })
    }
}
