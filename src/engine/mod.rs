pub mod mineru_engine;
pub mod mineru_runtime;
pub mod batch_queue;
pub mod model_manager;

use crate::models::ocr::{Cancellation, ProgressCallback};
use crate::models::task::{ConversionError, ConversionResult, ConversionTask};

/// Abstract document-to-markdown engine. OmniMD consumes documents through this
/// trait; MinerU is currently the only implementation (D4 decision: MinerU is
/// the sole parsing engine, the old anydoc/PP-OCRv6 stack is removed).
#[async_trait::async_trait]
pub trait DocumentEngine: Send + Sync {
    fn name(&self) -> &str;

    /// Whether the engine runtime is available right now (e.g. `mineru-api`
    /// subprocess is installed and reachable).
    fn is_available(&self) -> bool;

    /// Convert the source file of `task` into markdown.
    ///
    /// `on_progress` receives values in [0.0, 1.0] plus an optional detail
    /// string. `cancelled` allows cooperative cancellation; when set, the
    /// engine must stop at the next checkpoint and return
    /// `ErrorCode::Cancelled`.
    async fn convert(
        &self,
        task: &ConversionTask,
        on_progress: Option<ProgressCallback>,
        cancelled: Option<&Cancellation>,
    ) -> Result<ConversionResult, ConversionError>;
}
