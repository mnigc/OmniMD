import sys
sys.stdout.reconfigure(encoding="utf-8")

content = """use std::fs;
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

const DEFAULT_OCR_LANG: &str = "chi_sim+eng";

/// Progress reporter: maps a [0, 1] range to OCR progress.
/// `None` means no progress reporting.
pub type ProgressReporter = Option<ProgressCallback>;

/// Return `Err(Cancelled)` when the shared cancellation flag was triggered.
fn check_cancelled(cancelled: Option<&Cancellation>, stage: ConversionStage) -> Result<(), ConversionError> {
    if let Some(c) = cancelled {
        if c.cancelled() {
            return Err(ConversionError {
                code: ErrorCode::Cancelled,
                message: "任务已取消".to_string(),
                stage,
                retryable: false,
            });
        }
    }
    Ok(())
}

pub async fn convert_file(
    task: &ConversionTask,
    progress: ProgressReporter,
    cancelled: Option<&Cancellation>,
) -> Result<ConversionResult, ConversionError> {
    check_cancelled(cancelled, ConversionStage::DetectingFormat)?;

    let bytes = fs::read(&task.source_path).map_err(|e| ConversionError {
        code: ErrorCode::IoError,
        message: format!("Failed to read file: {}", e),
        stage: ConversionStage::DetectingFormat,
        retryable: true,
    })?;

    if file_utils::is_image_file(&task.source_path) {
        if task.ocr_mode == OcrMode::Off {
            return Err(ConversionError {
                code: ErrorCode::OcrFailed,
                message: "Image files require OCR support. Enable OCR in settings."
                    .to_string(),
                stage: ConversionStage::Ocr,
                retryable: false,
            });
        }
        check_cancelled(cancelled, ConversionStage::Ocr)?;
        return try_ocr_to_result(task, &bytes, progress, cancelled);
    }

    let converter = get_converter();
    let _detected_format = converter
        .detect_format(&bytes)
        .unwrap_or_else(|| "unknown".to_string());

    check_cancelled(cancelled, ConversionStage::Extracting)?;

    // Run the synchronous converter in a blocking thread with a timeout
    let convert_result = {
        let bytes = bytes.to_vec();
        tokio::time::timeout(Duration::from_secs(60), tokio::task::spawn_blocking(move || {
            converter.convert(&bytes)
        }))
        .await
    };

    let result = match convert_result {
        Ok(Ok(Ok(r))) => {
            if check_cancelled(cancelled, ConversionStage::Extracting).is_err() {
                tracing::info!("Conversion cancelled before write, discarding output");
                return Err(ConversionError {
                    code: ErrorCode::Cancelled,
                    message: "任务已取消".to_string(),
                    stage: ConversionStage::Extracting,
                    retryable: false,
                });
            }
            // Detect garbled text
            if task.ocr_mode != OcrMode::Off && is_text_garbled(&r.markdown) {
                tracing::warn!("PDF text extraction produced garbled output, falling back to OCR");
                return try_ocr_to_result(task, &bytes, progress, cancelled);
            }
            r
        }
        Ok(Ok(Err(e))) => {
            if check_cancelled(cancelled, ConversionStage::Extracting).is_err() {
                return Err(ConversionError {
                    code: ErrorCode::Cancelled,
                    message: "任务已取消".to_string(),
                    stage: ConversionStage::Extracting,
                    retryable: false,
                });
            }
            if e.message.contains("no extractable text") && task.ocr_mode != OcrMode::Off {
                return try_ocr_to_result(task, &bytes, progress, cancelled);
            }
            return Err(e);
        }
        Ok(Err(join_err)) => {
            if check_cancelled(cancelled, ConversionStage::Extracting).is_err() {
                return Err(ConversionError {
                    code: ErrorCode::Cancelled,
                    message: "任务已取消".to_string(),
                    stage: ConversionStage::Extracting,
                    retryable: false,
                });
            }
            return Err(ConversionError {
                code: ErrorCode::IoError,
                message: format!("Converter thread panicked: {}", join_err),
                stage: ConversionStage::Extracting,
                retryable: false,
            });
        }
        Err(_timeout) => {
            if check_cancelled(cancelled, ConversionStage::Extracting).is_err() {
                return Err(ConversionError {
                    code: ErrorCode::Cancelled,
                    message: "任务已取消".to_string(),
                    stage: ConversionStage::Extracting,
                    retryable: false,
                });
            }
            tracing::warn!("PDF text extraction timed out after 60s, falling back to OCR");
            if task.ocr_mode != OcrMode::Off {
                return try_ocr_to_result(task, &bytes, progress, cancelled);
            }
            return Err(ConversionError {
                code: ErrorCode::OcrFailed,
                message: "Text extraction timed out. Enable OCR for image-based PDFs.".to_string(),
                stage: ConversionStage::Extracting,
                retryable: true,
            });
        }
    };

    let mut result = result;
    result.task_id = task.id.clone();

    check_cancelled(cancelled, ConversionStage::Saving)?;

    result.markdown = markdown_pipeline::process(
        &result.markdown,
        &task.output_mode,
        &task.source_path,
        &task.ai_ready_opts,
    );

    write_result(task, &mut result)
}
"""

# Write the file
with open("src/pipeline.rs", "w", encoding="utf-8") as f:
    f.write(content)
print("Part 1 written, length:", len(content))
