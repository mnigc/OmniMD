use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::converters::get_converter;
use crate::file_utils;
use crate::markdown_pipeline;
use crate::models::converter::Converter;
use crate::models::ocr::{Cancellation, OcrEngine, OcrMode, ProgressCallback};
use crate::models::task::{
    ConversionError, ConversionResult, ConversionStats, ConversionStage, ConversionTask,
    ErrorCode, OutputMode,
};
use crate::ocr;
use crate::ocr::engine::is_pdf_bytes;
