pub mod mineru_engine;
pub mod mineru_runtime;
pub mod cloud_engine;
pub mod batch_queue;
pub mod model_manager;

use crate::models::ocr::{Cancellation, ProgressCallback};
use crate::models::task::{ConversionError, ConversionResult, ConversionTask};

/// Which engine the app uses for conversion: the bundled local MinerU
/// subprocess, or the MinerU Agent cloud API as a temporary fallback.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EngineMode {
    Local,
    Cloud,
}

impl Default for EngineMode {
    fn default() -> Self {
        EngineMode::Local
    }
}

impl EngineMode {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "cloud" => EngineMode::Cloud,
            _ => EngineMode::Local,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            EngineMode::Local => "local",
            EngineMode::Cloud => "cloud",
        }
    }
}

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
