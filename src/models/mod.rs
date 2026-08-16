pub mod document;
pub mod task;
pub mod converter;
pub mod ocr;

pub use document::{Block, Document, Asset};
pub use task::{
    ConversionTask, ConversionError, ConversionResult, ConversionStats, AiReadyOpts,
    TaskStatus, ConversionStage, ErrorCode, OutputMode,
};
pub use converter::Converter;
pub use ocr::{OcrEngine, OcrMode};
