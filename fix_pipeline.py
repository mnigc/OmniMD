import sys
with open("src/pipeline.rs", "r", encoding="utf-8") as f:
    content = f.read()

# Find the old try_ocr_to_result function
start = "fn try_ocr_to_result("
end = "    write_result(task, &mut result)\n}\n"

start_idx = content.find(start)
if start_idx < 0:
    print("ERROR: try_ocr_to_result not found")
    sys.exit(1)

end_idx = content.find(end, start_idx)
if end_idx < 0:
    print("ERROR: end marker not found")
    sys.exit(1)

end_idx = end_idx + len(end)
old_func = content[start_idx:end_idx]

new_func = """fn try_ocr_to_result(
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

    let text: String = ocr_result.blocks.iter().map(|b| b.text.as_str()).collect::<Vec<_>>().join("\\n");
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
                .split("/")
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
            Ok(r) => r.blocks.iter().map(|b| b.text.as_str()).collect::<Vec<_>>().join("\\n"),
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
            all_text.push('\\n');
        }
        all_text.push_str(text);
        all_text.push('\\n');
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
            file_name: task.source_path.split("/").last().unwrap_or("").to_string(),
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

fn cancelled_error() -> ConversionError {
    ConversionError {
        code: ErrorCode::Cancelled,
        message: "任务已取消".to_string(),
        stage: ConversionStage::Ocr,
        retryable: false,
    }
}
"""

content = content[:start_idx] + new_func + content[end_idx:]

# Update ConversionStats in write_result
old_stats = "    result.stats = Some(ConversionStats {\n        image_count: result.assets.len(),\n        table_count,\n        word_count,\n    });"

new_stats = "    result.stats = Some(ConversionStats {\n        image_count: result.assets.len(),\n        table_count,\n        word_count,\n        ocr_page_count: None,\n        ocr_char_count: None,\n        avg_confidence_permille: None,\n        low_confidence_count: None,\n    });"

content = content.replace(old_stats, new_stats)

with open("src/pipeline.rs", "w", encoding="utf-8") as f:
    f.write(content)
print("Done")
