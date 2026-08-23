pub mod anydoc_engine;
pub mod batch_queue;
pub mod model_manager;

use crate::models::ocr::{Cancellation, ProgressCallback};
use crate::models::task::{ConversionError, ConversionResult, ConversionTask};

/// Abstract document-to-markdown engine. OmniMD consumes documents through
/// this trait; `anydoc_engine` is the local implementation (pure Rust, no ML).
#[async_trait::async_trait]
pub trait DocumentEngine: Send + Sync {
    fn name(&self) -> &str;

    /// Whether the engine runtime is available right now.
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