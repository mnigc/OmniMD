import sys
sys.stdout.reconfigure(encoding="utf-8")

content = r"""
fn try_ocr_to_result(
    task: &ConversionTask,
    bytes: &[u8],
    progress: ProgressReporter,
    cancelled: Option<&Cancellation>,
) -> Result<ConversionResult, ConversionError> {
    let engine = ocr::get_engine();

    check_cancelled(cancelled, ConversionStage::Ocr)?;

    let config = crate::models::ocr::OcrConfig {
        mode: task.ocr_mode,
        language: DEFAULT_OCR_LANG.to_string(),
        ..Default::default()
    };

    // For PDF files, render pages to images and OCR each page
    if is_pdf_bytes(bytes) {
        return ocr_pdf_images(task, bytes, &engine, &config, progress, cancelled);
    }

    // For regular image files, OCR directly
    let ocr_result = engine.recognize_image(
        bytes,
        &config,
        progress,
        cancelled,
    )
    .map_err(|e| {
        if e.code == ErrorCode::Cancelled {
            return e;
        }
        if e.message.contains("not available") {
            ConversionError {
                code: ErrorCode::OcrFailed,
                message: "OCR engine not available. PP-OCRv6 resources are missing."
                    .to_string(),
                stage: ConversionStage::Ocr,
                retryable: false,
            }
        } else {
            ConversionError {
                code: ErrorCode::OcrFailed,
                message: e.message,
                stage: ConversionStage::Ocr,
                retryable: e.retryable,
            }
        }
    })?;

    check_cancelled(cancelled, ConversionStage::Ocr)?;

    let text: String = ocr_result.blocks.iter().map(|b| b.text.as_str()).collect::<Vec<_>>().join("\n");
    let markdown = text.trim().to_string();
    if markdown.is_empty() {
        return Err(ConversionError {
            code: ErrorCode::OcrFailed,
            message: "OCR produced no output. The file may not contain readable text."
                .to_string(),
            stage: ConversionStage::Ocr,
            retryable: true,
        });
    }

    let processed = markdown_pipeline::process(
        &markdown,
        &task.output_mode,
        &task.source_path,
        &task.ai_ready_opts,
    );

    let doc = crate::models::document::Document {
        metadata: crate::models::document::DocumentMetadata {
            file_name: task
                .source_path
                .split('/')
                .last()
                .unwrap_or("")
                .to_string(),
            format: "ocr".to_string(),
            size_bytes: bytes.len() as u64,
            converted_at: String::new(),
        },
        blocks: vec![crate::models::document::Block::Paragraph {
            content: processed.clone(),
        }],
        assets: Vec::new(),
    };

    let mut result = ConversionResult {
        task_id: task.id.clone(),
        markdown: processed,
        document: doc,
        assets: Vec::new(),
        errors: Vec::new(),
        output_path: String::new(),
        stats: None,
    };

    check_cancelled(cancelled, ConversionStage::Saving)?;

    write_result(task, &mut result)
}

/// OCR a PDF by rendering each page to an image at 200 DPI using PDFium,
/// then recognizing text on each page.
fn ocr_pdf_images(
    task: &ConversionTask,
    bytes: &[u8],
    engine: &impl OcrEngine,
    config: &crate::models::ocr::OcrConfig,
    progress: ProgressReporter,
    cancelled: Option<&Cancellation>,
) -> Result<ConversionResult, ConversionError> {
    use pdfium_render::prelude::*;

    if let Some(c) = cancelled {
        if c.cancelled() {
            return Err(cancelled_error());
        }
    }

    // Bind to PDFium (auto-downloads on first use, cached afterwards)
    let pdfium = pdfium_bundled::bind_pdfium_silent().map_err(|e| ConversionError {
        code: ErrorCode::OcrFailed,
        message: format!("Failed to initialize PDF renderer: {}", e),
        stage: ConversionStage::Ocr,
        retryable: false,
    })?;

    let document = pdfium.load_pdf_from_byte_slice(bytes, None).map_err(|e| ConversionError {
        code: ErrorCode::OcrFailed,
        message: format!("Failed to load PDF: {}", e),
        stage: ConversionStage::Ocr,
        retryable: false,
    })?;

    let total_pages = document.pages().len();
    if total_pages == 0 {
        return Err(ConversionError {
            code: ErrorCode::OcrFailed,
            message: "PDF has no pages.".to_string(),
            stage: ConversionStage::Ocr,
            retryable: false,
        });
    }

    let mut all_text = String::new();

    for page_index in 0..total_pages {
        if let Some(ref cb) = progress {
            let p = (page_index as f32) / (total_pages as f32) * 0.95;
            cb(p);
        }

        if let Some(c) = cancelled {
            if c.cancelled() {
                return Err(cancelled_error());
            }
        }

        let page = document.pages().get(page_index).map_err(|e| ConversionError {
            code: ErrorCode::OcrFailed,
            message: format!("Failed to get PDF page {}: {}", page_index + 1, e),
            stage: ConversionStage::Ocr,
            retryable: false,
        })?;

        // Render at 200 DPI for good OCR quality
        let width_pts = page.get_width();
        let height_pts = page.get_height();
        let dpi_scale = 200.0 / 72.0;
        let target_width = (width_pts * dpi_scale) as i32;
        let target_height = (height_pts * dpi_scale) as i32;

        let render_config = PdfRenderConfig::new()
            .set_target_width(target_width)
            .set_target_height(target_height);

        let bitmap = page.render_with_config(&render_config).map_err(|e| ConversionError {
            code: ErrorCode::OcrFailed,
            message: format!("Failed to render PDF page {}: {}", page_index + 1, e),
            stage: ConversionStage::Ocr,
            retryable: false,
        })?;

        // Convert rendered bitmap to PNG bytes for OCR
        let image = bitmap.as_image().map_err(|_| ConversionError {
            code: ErrorCode::OcrFailed,
            message: "Failed to convert PDF page render to image.".to_string(),
            stage: ConversionStage::Ocr,
            retryable: false,
        })?;

        let mut png_bytes = Vec::new();
        image.write_to(&mut std::io::Cursor::new(&mut png_bytes), image::ImageFormat::Png)
            .map_err(|_| ConversionError {
                code: ErrorCode::OcrFailed,
                message: "Failed to encode PDF page as PNG.".to_string(),
                stage: ConversionStage::Ocr,
                retryable: false,
            })?;

        // Release page and bitmap before OCR to free memory
        drop(bitmap);
        drop(page);

        let page_result = engine.recognize_image(&png_bytes, config, None, cancelled);

        let text = match page_result {
            Ok(r) => r.blocks.iter().map(|b| b.text.as_str()).collect::<Vec<_>>().join("\n"),
            Err(e) => {
                if e.code == ErrorCode::Cancelled {
                    return Err(e);
                }
                tracing::warn!("OCR failed on PDF page {}: {}", page_index + 1, e.message);
                String::new()
            }
        };

        let text = text.trim();
        if text.is_empty() {
            continue;
        }

        if !all_text.is_empty() {
            all_text.push('\n');
        }
        all_text.push_str(text);
        all_text.push('\n');
    }

    if let Some(ref cb) = progress {
        cb(1.0);
    }

    let markdown = all_text.trim().to_string();
    if markdown.is_empty() {
        return Err(ConversionError {
            code: ErrorCode::OcrFailed,
            message: "OCR produced no output from the PDF pages.".to_string(),
            stage: ConversionStage::Ocr,
            retryable: true,
        });
    }

    let processed = markdown_pipeline::process(
        &markdown,
        &task.output_mode,
        &task.source_path,
        &task.ai_ready_opts,
    );

    let doc = crate::models::document::Document {
        metadata: crate::models::document::DocumentMetadata {
            file_name: task.source_path.split('/').last().unwrap_or("").to_string(),
            format: "ocr".to_string(),
            size_bytes: bytes.len() as u64,
            converted_at: String::new(),
        },
        blocks: vec![crate::models::document::Block::Paragraph {
            content: processed.clone(),
        }],
        assets: Vec::new(),
    };

    let mut result = ConversionResult {
        task_id: task.id.clone(),
        markdown: processed,
        document: doc,
        assets: Vec::new(),
        errors: Vec::new(),
        output_path: String::new(),
        stats: None,
    };

    check_cancelled(cancelled, ConversionStage::Saving)?;

    write_result(task, &mut result)
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
        })?;
    }

    fs::write(&md_path, &result.markdown).map_err(|e| ConversionError {
        code: ErrorCode::IoError,
        message: format!("Failed to write markdown: {}", e),
        stage: ConversionStage::Saving,
        retryable: true,
    })?;

    if let Some(asset_dir) = &asset_dir_opt {
        fs::create_dir_all(asset_dir).map_err(|e| ConversionError {
            code: ErrorCode::IoError,
            message: format!("Failed to create assets directory: {}", e),
            stage: ConversionStage::Saving,
            retryable: false,
        })?;

        for asset in &result.assets {
            let asset_path = asset_dir.join(&asset.name);
            let resolved_path = file_utils::resolve_unique_path(asset_path);
            fs::write(&resolved_path, &asset.bytes).map_err(|e| ConversionError {
                code: ErrorCode::IoError,
                message: format!("Failed to write asset {}: {}", asset.name, e),
                stage: ConversionStage::Saving,
                retryable: true,
            })?;
        }
    }

    let table_count = markdown_pipeline::count_table_separators(&result.markdown);
    let word_count = markdown_pipeline::count_words(&result.markdown);
    result.output_path = md_path.to_string_lossy().to_string();
    result.stats = Some(ConversionStats {
        image_count: result.assets.len(),
        table_count,
        word_count,
        ocr_page_count: None,
        ocr_char_count: None,
        avg_confidence_permille: None,
        low_confidence_count: None,
    });

    Ok(result.clone())
}

fn cancelled_error() -> ConversionError {
    ConversionError {
        code: ErrorCode::Cancelled,
        message: "任务已取消".to_string(),
        stage: ConversionStage::Ocr,
        retryable: false,
    }
}

/// Derive the effective output directory (parent of the markdown file) from
/// a tentative `output_path`. Falls back to "." when there is no parent.
fn effective_output_dir(output_path: &str) -> String {
    let p = Path::new(output_path);
    match p.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => {
            parent.to_string_lossy().to_string()
        }
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
fn is_text_garbled(text: &str) -> bool {
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
"""

with open("src/pipeline.rs", "a", encoding="utf-8") as f:
    f.write(content)
print("Part 2 appended, total length:", len(content))
