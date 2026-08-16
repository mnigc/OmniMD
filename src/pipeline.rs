use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::converters::get_converter;
use crate::file_utils;
use crate::markdown_pipeline;
use crate::models::converter::Converter;
use crate::models::ocr::{Cancellation, OcrConfig, OcrEngine, OcrMode, OcrBlock, ProgressCallback};
use crate::models::task::{
    ConversionError, ConversionResult, ConversionStats, ConversionStage, ConversionTask,
    ErrorCode, OutputMode,
};
use crate::ocr;
use crate::ocr::pdf_renderer::is_pdf;

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
                message: "?????".to_string(),
                stage,
                retryable: false,
                page: None,
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
        page: None,
    })?;

    if file_utils::is_image_file(&task.source_path) {
        if task.ocr_mode == OcrMode::Off {
            return Err(ConversionError {
                code: ErrorCode::OcrFailed,
                message: "Image files require OCR support. Enable OCR in settings."
                    .to_string(),
                stage: ConversionStage::Ocr,
                retryable: false,
                page: None,
            });
        }
        check_cancelled(cancelled, ConversionStage::Ocr)?;
        return ocr_image_to_result(task, &bytes, progress, cancelled);
    }

    if is_pdf(&bytes) {
        match task.ocr_mode {
            OcrMode::Off => return convert_via_anydoc(task, &bytes, progress, cancelled).await,
            OcrMode::Auto | OcrMode::Always => {
                return convert_hybrid_pdf(task, &bytes, task.ocr_mode, progress, cancelled)
                    .await
            }
        }
    }

    // Non-PDF document formats: text/structural extraction via anydoc.
    convert_via_anydoc(task, &bytes, progress, cancelled).await
}

/// Convert a standalone image via OCR (single page).
fn ocr_image_to_result(
    task: &ConversionTask,
    bytes: &[u8],
    progress: ProgressReporter,
    cancelled: Option<&Cancellation>,
) -> Result<ConversionResult, ConversionError> {
    let engine = ocr::get_engine();
    check_cancelled(cancelled, ConversionStage::Ocr)?;

    let config = OcrConfig {
        mode: task.ocr_mode,
        language: DEFAULT_OCR_LANG.to_string(),
        ..Default::default()
    };

    if let Some(cb) = &progress {
        cb(0.05, Some("???????".to_string()));
    }
    let preprocessed = ocr::preprocess::preprocess_image(bytes).map_err(|e| {
        if e.code == ErrorCode::Cancelled {
            return e;
        }
        // Fall back to the raw bytes if preprocessing fails.
        tracing::warn!("Image preprocessing failed, using original: {}", e.message);
        ConversionError {
            code: ErrorCode::OcrFailed,
            message: String::new(),
            stage: ConversionStage::Ocr,
            retryable: true,
            page: None,
        }
    });
    let image_bytes: &[u8] = match &preprocessed {
        Ok(p) => p,
        Err(_) => bytes,
    };

    let ocr_result = engine.recognize_image(image_bytes, &config, progress, cancelled)?;
    check_cancelled(cancelled, ConversionStage::Ocr)?;

    if ocr_result.blocks.is_empty()
        || ocr_result.blocks.iter().all(|b| b.text.trim().is_empty())
    {
        return Err(ConversionError {
            code: ErrorCode::OcrFailed,
            message: "OCR produced no output. The file may not contain readable text."
                .to_string(),
            stage: ConversionStage::Ocr,
            retryable: true,
            page: None,
        });
    }

    let blocks = ocr::postprocess::process_ocr_result(&ocr_result.blocks);
    let rendered = ocr::postprocess::render_document(&blocks);
    let processed = markdown_pipeline::process(
        &rendered,
        &task.output_mode,
        &task.source_path,
        &task.ai_ready_opts,
    );

    let (char_count, avg_conf, low_conf) = ocr_conf_stats(&ocr_result.blocks);

    let document = ocr::postprocess::blocks_to_document(
        blocks,
        &task.source_path,
        "ocr",
        bytes.len() as u64,
    );

    let mut result = ConversionResult {
        task_id: task.id.clone(),
        markdown: processed,
        document,
        assets: Vec::new(),
        errors: Vec::new(),
        output_path: String::new(),
        stats: Some(ConversionStats {
            ocr_page_count: Some(1),
            ocr_char_count: Some(char_count),
            avg_confidence_permille: avg_conf,
            low_confidence_count: Some(low_conf),
            ..Default::default()
        }),
    };

    check_cancelled(cancelled, ConversionStage::Saving)?;
    write_result(task, &mut result)
}

/// Hybrid PDF conversion (T2/T7): per-page text-layer detection, native text
/// extraction where available, OCR where not.
async fn convert_hybrid_pdf(
    task: &ConversionTask,
    bytes: &[u8],
    ocr_mode: OcrMode,
    progress: ProgressReporter,
    cancelled: Option<&Cancellation>,
) -> Result<ConversionResult, ConversionError> {
    let engine = ocr::get_engine();
    let config = OcrConfig {
        mode: ocr_mode,
        language: DEFAULT_OCR_LANG.to_string(),
        ..Default::default()
    };
    let dpi = 200u32;

    check_cancelled(cancelled, ConversionStage::Ocr)?;

    // Native text extraction for the whole document (per-page routing).
    let native_items = ocr::pdf_text_detection::extract_native_items(bytes);
    let force_ocr = ocr_mode == OcrMode::Always || native_items.is_err();

    // Always get the true page count from PDFium. Deriving it from
    // pdf-inspector text items (max page number among items) undercounts when
    // trailing pages lack a native text layer (scanned / image-only pages),
    // silently dropping those pages from the output.
    let page_count = pdf_page_count(bytes)?;
    
    // DETAILED DIAGNOSTIC LOGGING FOR PAGE COUNT ISSUE
    tracing::info!(
        "convert_hybrid_pdf START: source_path={}, pdfium_page_count={}, native_items_page_count={}, native_item_count={}, force_ocr={}, ocr_mode={:?}",
        task.source_path,
        page_count,
        native_items
            .as_ref()
            .map(|i| ocr::pdf_text_detection::page_count(i))
            .unwrap_or(0),
        native_items.as_ref().map(|i| i.len()).unwrap_or(0),
        force_ocr,
        task.ocr_mode,
    );
    
    if page_count == 0 {
        return Err(ConversionError {
            code: ErrorCode::OcrFailed,
            message: "PDF has no pages.".to_string(),
            stage: ConversionStage::Ocr,
            retryable: false,
            page: None,
        });
    }

    // Fast path (Auto, native text on every page): use anydoc for richer layout.
    if !force_ocr {
        if let Ok(items) = &native_items {
            let total = page_count as u32;
            let all_text = (1..=total).all(|p| ocr::pdf_text_detection::has_native_text(items, p));
            if all_text {
                tracing::debug!(
                    "convert_hybrid_pdf: all {} pages have native text — using anydoc fast path",
                    total
                );
                let mut r = anydoc_convert_only(task, bytes, progress.clone(), cancelled).await?;
                return write_result(task, &mut r);
            }
        }
    }

    let mut all_blocks: Vec<OcrBlock> = Vec::new();
    let mut errors: Vec<ConversionError> = Vec::new();
    let mut ocr_pages = 0usize;
    let mut char_count = 0usize;
    let mut total_conf = 0.0f32;
    let mut conf_count = 0usize;
    let mut low_conf = 0usize;

    for page in 0..page_count {
        let page_no = (page + 1) as u32;
        
        // PER-PAGE DIAGNOSTIC LOGGING
        tracing::info!("convert_hybrid_pdf: processing page {}/{} (index={})", page_no, page_count, page);

        if !force_ocr {
            if let Ok(items) = &native_items {
                if ocr::pdf_text_detection::has_native_text(items, page_no) {
                    let blocks = ocr::pdf_text_detection::ocr_blocks_from_items(items, page_no);
                    tracing::info!(
                        "convert_hybrid_pdf: page {}/{} has native text ({} blocks), using extracted text",
                        page_no, page_count, blocks.len()
                    );
                    all_blocks.extend(blocks);
                    continue;
                } else {
                    tracing::info!(
                        "convert_hybrid_pdf: page {}/{} has NO native text, will OCR",
                        page_no, page_count
                    );
                }
            }
        }

        tracing::info!(
            "convert_hybrid_pdf: page {}/{} will be OCR'd (no native text or force_ocr={})",
            page_no, page_count, force_ocr
        );

        if let Some(cb) = &progress {
            let p = 0.4 + (page as f32 / page_count as f32) * 0.5;
            cb(
                p,
                Some(format!("?? OCR ? {} / {} ?", page_no, page_count)),
            );
        }

        if let Some(c) = cancelled {
            if c.cancelled() {
                return Err(cancelled_error());
            }
        }

        let png = match ocr::pdf_renderer::render_pdf_page_to_png(bytes, page, dpi) {
            Ok(p) => {
                tracing::info!("convert_hybrid_pdf: page {}/{} rendered to PNG ({} bytes)", page_no, page_count, p.len());
                p
            }
            Err(e) => {
                tracing::error!("convert_hybrid_pdf: page {}/{} render FAILED: {}", page_no, page_count, e.message);
                if e.code == ErrorCode::Cancelled {
                    return Err(e);
                }
                errors.push(ConversionError {
                    code: ErrorCode::OcrFailed,
                    message: format!("渲染页面 {} 失败: {}", page_no, e.message),
                    stage: ConversionStage::Ocr,
                    retryable: false,
                    page: Some(page_no),
                });
                continue;
            }
        };

        let png_clone = png.clone();
        let preprocessed = ocr::preprocess::preprocess_image(&png).unwrap_or(png_clone);
        let png_len = preprocessed.len();

        let mut ocr_result = engine.recognize_image(&preprocessed, &config, None, cancelled);

        // If OCR on the preprocessed image returns no blocks, retry with the
        // original rendered PNG — the box-blur + contrast stretch can suppress
        // fine text on clean 200 DPI renders and cause PP-OCRv6 to find nothing.
        if let Ok(ref r) = ocr_result {
            if r.blocks.is_empty() {
                tracing::warn!(
                    "OCR produced 0 blocks on PDF page {} (preprocessed {} bytes); \
                     retrying with original",
                    page_no,
                    png_len
                );
                ocr_result = engine.recognize_image(&png, &config, None, cancelled);
            }
        }

        match ocr_result {
            Ok(mut r) => {
                r.page = page_no;
                // Propagate the correct page number to individual blocks.
                // recognize_image() hardcodes page=1 on every OcrBlock, so
                // without this all OCR'd blocks from every page collapse
                // into the page-1 grouping in process_ocr_result(), jumbling
                // cross-page reading order.
                for b in &mut r.blocks {
                    b.page = page_no;
                }
                let block_count = r.blocks.len();
                let char_count_page: usize = r.blocks.iter().map(|b| b.text.chars().count()).sum();
                tracing::info!(
                    "convert_hybrid_pdf: page {}/{} OCR SUCCEEDED ({} blocks, {} chars)",
                    page_no, page_count, block_count, char_count_page
                );
                for b in &r.blocks {
                    char_count += b.text.chars().count();
                    total_conf += b.confidence;
                    conf_count += 1;
                    if b.confidence < 0.5 {
                        low_conf += 1;
                    }
                }
                all_blocks.extend(r.blocks);
                ocr_pages += 1;
            }
            Err(e) => {
                if e.code == ErrorCode::Cancelled {
                    return Err(e);
                }
                tracing::error!("convert_hybrid_pdf: page {}/{} OCR FAILED: {}", page_no, page_count, e.message);
                errors.push(ConversionError {
                    code: ErrorCode::OcrFailed,
                    message: format!("页面 {} OCR 失败: {}", page_no, e.message),
                    stage: ConversionStage::Ocr,
                    retryable: e.retryable,
                    page: Some(page_no),
                });
            }
        }
    }
    
    // LOOP COMPLETION DIAGNOSTIC LOGGING
    tracing::info!(
        "convert_hybrid_pdf LOOP COMPLETE: processed {} pages, all_blocks={}, ocr_pages={}, errors={}",
        page_count, all_blocks.len(), ocr_pages, errors.len()
    );

    if all_blocks.is_empty() {
        let page_errors: Vec<String> = errors
            .iter()
            .map(|e| {
                let page = e
                    .page
                    .map(|p| format!("page {}", p))
                    .unwrap_or_else(|| "unknown page".to_string());
                format!("{}: {}", page, e.message)
            })
            .collect();

        let detail = if page_errors.is_empty() {
            String::from(
                "No text was detected on any page (OCR returned empty results). \
                 This may happen when the document is a blank scan, the image quality \
                 is too low, or the models could not decode the content.",
            )
        } else {
            format!(
                "Per-page failures: {}. \
                 Also verify that the PP-OCRv6 model resources are present in \
                 ocr_resources/ppocrv6/ and that onnxruntime is loadable.",
                page_errors.join("; ")
            )
        };

        tracing::warn!(
            "OCR produced no output across all {} pages; {} errors collected",
            page_count,
            errors.len()
        );

        return Err(ConversionError {
            code: ErrorCode::OcrFailed,
            message: format!("OCR produced no output from the document. {}", detail),
            stage: ConversionStage::Ocr,
            retryable: true,
            page: None,
        });
    }

    let blocks = ocr::postprocess::process_ocr_result(&all_blocks);
    let rendered = ocr::postprocess::render_document(&blocks);
    let processed = markdown_pipeline::process(
        &rendered,
        &task.output_mode,
        &task.source_path,
        &task.ai_ready_opts,
    );

    let document = ocr::postprocess::blocks_to_document(
        blocks,
        &task.source_path,
        "ocr",
        bytes.len() as u64,
    );

    let mut result = ConversionResult {
        task_id: task.id.clone(),
        markdown: processed,
        document,
        assets: Vec::new(),
        errors,
        output_path: String::new(),
        stats: Some(ConversionStats {
            ocr_page_count: Some(ocr_pages),
            ocr_char_count: Some(char_count),
            avg_confidence_permille: if conf_count > 0 {
                Some((total_conf / conf_count as f32 * 1000.0) as u32)
            } else {
                None
            },
            low_confidence_count: Some(low_conf),
            ..Default::default()
        }),
    };

    check_cancelled(cancelled, ConversionStage::Saving)?;
    write_result(task, &mut result)
}

/// Pure anydoc conversion: text/structure extraction + markdown pipeline, but
/// NO OCR fallback. The caller decides whether to fall back to OCR.
async fn anydoc_convert_only(
    task: &ConversionTask,
    bytes: &[u8],
    _progress: ProgressReporter,
    cancelled: Option<&Cancellation>,
) -> Result<ConversionResult, ConversionError> {
    let converter = get_converter();
    let _detected_format = converter
        .detect_format(bytes)
        .unwrap_or_else(|| "unknown".to_string());

    check_cancelled(cancelled, ConversionStage::Extracting)?;

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
                return Err(cancelled_error());
            }
            r
        }
        Ok(Ok(Err(e))) => return Err(e),
        Ok(Err(join_err)) => {
            return Err(ConversionError {
                code: ErrorCode::IoError,
                message: format!("Converter thread panicked: {}", join_err),
                stage: ConversionStage::Extracting,
                retryable: false,
                page: None,
            });
        }
        Err(_timeout) => {
            return Err(ConversionError {
                code: ErrorCode::OcrFailed,
                message: "Text extraction timed out.".to_string(),
                stage: ConversionStage::Extracting,
                retryable: true,
                page: None,
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

    Ok(result)
}

/// Run the anydoc converter with the garbled / empty / timed-out fallback to
/// full-page OCR (used for PDFs and generic document formats).
async fn convert_via_anydoc(
    task: &ConversionTask,
    bytes: &[u8],
    progress: ProgressReporter,
    cancelled: Option<&Cancellation>,
) -> Result<ConversionResult, ConversionError> {
    let result = match anydoc_convert_only(task, bytes, progress.clone(), cancelled).await {
        Ok(r) => r,
        Err(e) => {
            if e.message.contains("timed out") && task.ocr_mode != OcrMode::Off {
                tracing::warn!("Text extraction timed out after 60s, falling back to OCR");
                return convert_hybrid_pdf(task, bytes, OcrMode::Always, progress, cancelled).await;
            }
            return Err(e);
        }
    };

    let empty_or_garbled = result.markdown.trim().is_empty() || is_text_garbled(&result.markdown);
    if task.ocr_mode != OcrMode::Off && empty_or_garbled {
        tracing::warn!("PDF text extraction produced garbled/empty output, falling back to OCR");
        return convert_hybrid_pdf(task, bytes, OcrMode::Always, progress, cancelled).await;
    }

    let mut result = result;
    write_result(task, &mut result)
}

/// Count pages in a PDF using PDFium, with lopdf as fallback.
fn pdf_page_count(bytes: &[u8]) -> Result<usize, ConversionError> {
    // Primary: PDFium
    let pdfium_result = (|| -> Result<usize, ConversionError> {
        let pdfium = ocr::pdf_renderer::get_pdfium()?;
        let document = pdfium.load_pdf_from_byte_slice(bytes, None).map_err(|e| ConversionError {
            code: ErrorCode::OcrFailed,
            message: format!("Failed to load PDF with PDFium: {}", e),
            stage: ConversionStage::Ocr,
            retryable: false,
            page: None,
        })?;
        let count = document.pages().len() as usize;
        tracing::debug!("pdf_page_count: PDFium reported {} pages", count);
        Ok(count)
    })();

    match pdfium_result {
        Ok(count) if count > 0 => {
            tracing::info!("pdf_page_count: using PDFium count = {}", count);
            Ok(count)
        }
        Ok(count) => {
            tracing::warn!("pdf_page_count: PDFium returned suspicious count = {}, trying lopdf fallback", count);
            pdf_page_count_lopdf(bytes)
        }
        Err(e) => {
            tracing::warn!("pdf_page_count: PDFium failed: {}, trying lopdf fallback", e.message);
            pdf_page_count_lopdf(bytes)
        }
    }
}

/// Fallback page count using lopdf.
fn pdf_page_count_lopdf(bytes: &[u8]) -> Result<usize, ConversionError> {
    use lopdf::Document;
    let doc = Document::load_mem(bytes).map_err(|e| ConversionError {
        code: ErrorCode::OcrFailed,
        message: format!("Failed to load PDF with lopdf: {}", e),
        stage: ConversionStage::Ocr,
        retryable: false,
        page: None,
    })?;
    let count = doc.get_pages().len();
    tracing::info!("pdf_page_count: using lopdf fallback count = {}", count);
    Ok(count)
}

/// Aggregate OCR confidence statistics for a set of blocks.
fn ocr_conf_stats(blocks: &[OcrBlock]) -> (usize, Option<u32>, usize) {
    let mut chars = 0usize;
    let mut sum = 0.0f32;
    let mut count = 0usize;
    let mut low = 0usize;
    for b in blocks {
        chars += b.text.chars().count();
        sum += b.confidence;
        count += 1;
        if b.confidence < 0.5 {
            low += 1;
        }
    }
    let avg = if count > 0 {
        Some((sum / count as f32 * 1000.0) as u32)
    } else {
        None
    };
    (chars, avg, low)
}

fn write_result(
    task: &ConversionTask,
    result: &mut ConversionResult,
) -> Result<ConversionResult, ConversionError> {
    let effective_dir = effective_output_dir(&task.output_path);
    let has_assets = !result.assets.is_empty();
    let (md_path, asset_dir_opt): (PathBuf, Option<PathBuf>) =
        file_utils::get_output_path_with_assets(&task.source_path, &effective_dir, has_assets);

    let output_dir = md_path.parent().unwrap_or_else(|| Path::new("."));
    if !output_dir.exists() {
        fs::create_dir_all(output_dir).map_err(|e| ConversionError {
            code: ErrorCode::IoError,
            message: format!("Failed to create output directory: {}", e),
            stage: ConversionStage::Saving,
            retryable: false,
            page: None,
        })?;
    }

    fs::write(&md_path, &result.markdown).map_err(|e| ConversionError {
        code: ErrorCode::IoError,
        message: format!("Failed to write markdown: {}", e),
        stage: ConversionStage::Saving,
        retryable: true,
        page: None,
    })?;

    if let Some(asset_dir) = &asset_dir_opt {
        fs::create_dir_all(asset_dir).map_err(|e| ConversionError {
            code: ErrorCode::IoError,
            message: format!("Failed to create assets directory: {}", e),
            stage: ConversionStage::Saving,
            retryable: false,
            page: None,
        })?;

        for asset in &result.assets {
            let asset_path = asset_dir.join(&asset.name);
            let resolved_path = file_utils::resolve_unique_path(asset_path);
            fs::write(&resolved_path, &asset.bytes).map_err(|e| ConversionError {
                code: ErrorCode::IoError,
                message: format!("Failed to write asset {}: {}", asset.name, e),
                stage: ConversionStage::Saving,
                retryable: true,
                page: None,
            })?;
        }
    }

    let table_count = markdown_pipeline::count_table_separators(&result.markdown);
    let word_count = markdown_pipeline::count_words(&result.markdown);

    // Preserve OCR stats that were filled by the OCR paths; only compute the
    // generic (non-OCR) fields here.
    let mut stats = result.stats.clone().unwrap_or_default();
    stats.image_count = result.assets.len();
    stats.table_count = table_count;
    stats.word_count = word_count;
    result.stats = Some(stats);

    result.output_path = md_path.to_string_lossy().to_string();

    Ok(result.clone())
}

fn cancelled_error() -> ConversionError {
    ConversionError {
        code: ErrorCode::Cancelled,
        message: "?????".to_string(),
        stage: ConversionStage::Ocr,
        retryable: false,
        page: None,
    }
}

/// Derive the effective output directory (parent of the markdown file) from
/// a tentative `output_path`. Falls back to "." when there is no parent.
fn effective_output_dir(output_path: &str) -> String {
    let p = Path::new(output_path);
    match p.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent.to_string_lossy().to_string(),
        _ => ".".to_string(),
    }
}

/// Collect conversion tasks for a folder.
pub fn collect_folder_tasks(
    source_dir: &str,
    output_dir: &str,
    mode: OutputMode,
    recursive: bool,
    keep_structure: bool,
) -> Vec<ConversionTask> {
    let extensions: &[&str] = &[
        "pdf", "docx", "doc", "pptx", "ppt", "xlsx", "xls", "epub", "csv",
        "txt", "html", "htm", "odt", "ods", "odp", "rtf", "png", "jpg", "jpeg",
        "tiff", "tif", "bmp",
    ];
    let files = if recursive {
        file_utils::list_files_recursive(source_dir, extensions)
    } else {
        file_utils::list_files_flat(source_dir, extensions)
    };

    let source_root = Path::new(source_dir);
    let output_prefix = normalize_dir(output_dir);

    files
        .into_iter()
        .map(|file_path| {
            let stem = file_path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("output");
            let md_name = format!("{}.md", stem);

            let relative = file_path
                .strip_prefix(source_root)
                .unwrap_or(&file_path)
                .to_path_buf();

            let output_path = if keep_structure {
                if let Some(parent) = relative.parent() {
                    if !parent.as_os_str().is_empty() {
                        format!("{}/{}", output_prefix, parent.join(md_name).to_string_lossy())
                    } else {
                        format!("{}/{}", output_prefix, md_name)
                    }
                } else {
                    format!("{}/{}", output_prefix, md_name)
                }
            } else {
                format!("{}/{}", output_prefix, md_name)
            };

            ConversionTask::with_mode(
                file_path.to_string_lossy().as_ref(),
                &output_path,
                mode.clone(),
            )
        })
        .collect()
}

/// Check whether the given text is likely garbled (contains too many
/// replacement characters U+FFFD, which indicate invalid UTF-8 decoding).
pub(crate) fn is_text_garbled(text: &str) -> bool {
    let total = text.len();
    if total < 20 {
        return false;
    }

    let replacement_count = text.chars().filter(|&c| c == '\u{FFFD}').count();
    if replacement_count < 10 {
        return false;
    }

    let ratio = replacement_count as f64 / total as f64;
    ratio > 0.05
}

/// Normalize a directory string for joining: forward slashes, no trailing slash.
fn normalize_dir(dir: &str) -> String {
    dir.replace('\\', "/").trim_end_matches('/').to_string()
}
