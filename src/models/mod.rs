pub mod document;
pub mod ocr;
pub mod task;

pub use document::{Block, Document, Asset};
pub use task::{
    ConversionTask, ConversionError, ConversionResult, ConversionStats, AiReadyOpts,
    TaskStatus, ConversionStage, ErrorCode, OutputMode, ParseQuality,
};
