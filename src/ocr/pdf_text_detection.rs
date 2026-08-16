//! Per-page text-layer detection for hybrid PDFs (T2).
//!
//! Uses `pdf-inspector` to extract positioned text items for the whole document,
//! then groups them per page. The pipeline uses this to decide, for each page,
//! whether a native text layer is present (and trustworthy) or whether the page
//! must be rendered and OCR'd.

use std::collections::BTreeMap;

use crate::models::ocr::OcrBlock;
use crate::models::task::{ConversionError, ConversionStage, ErrorCode};

/// Minimum number of characters on a page for it to count as having a real
/// text layer (avoids treating stray labels as body text).
const MIN_NATIVE_CHARS: usize = 30;

/// Extract all positioned text items from a PDF buffer.
pub fn extract_native_items(bytes: &[u8]) -> Result<Vec<pdf_inspector::types::TextItem>, ConversionError> {
    pdf_inspector::extractor::extract_text_with_positions_mem_pages(bytes, None).map_err(|e| {
        ConversionError {
            code: ErrorCode::OcrFailed,
            message: format!("Native text extraction failed: {}", e),
            stage: ConversionStage::Extracting,
            retryable: false,
            page: None,
        }
    })
}

/// Group text items by 1-based page number.
pub fn group_by_page(
    items: &[pdf_inspector::types::TextItem],
) -> BTreeMap<u32, Vec<pdf_inspector::types::TextItem>> {
    let mut map: BTreeMap<u32, Vec<pdf_inspector::types::TextItem>> = BTreeMap::new();
    for it in items {
        map.entry(it.page).or_default().push(it.clone());
    }
    map
}

/// Total number of pages (max page index seen), or 0 when nothing was extracted.
pub fn page_count(items: &[pdf_inspector::types::TextItem]) -> usize {
    items.iter().map(|i| i.page).max().unwrap_or(0) as usize
}

/// Whether a page has a usable native text layer (enough text, not garbled).
pub fn has_native_text(items: &[pdf_inspector::types::TextItem], page: u32) -> bool {
    let text: String = items
        .iter()
        .filter(|i| i.page == page)
        .map(|i| i.text.as_str())
        .collect();
    if text.trim().chars().count() < MIN_NATIVE_CHARS {
        return false;
    }
    !crate::pipeline::is_text_garbled(&text)
}

/// Convert a page's native text items into `OcrBlock`s so they flow through the
/// same post-processing pipeline as OCR results. Coordinates use a top-left
/// origin (image-style) so reading-order/out heuristics behave consistently.
pub fn ocr_blocks_from_items(
    items: &[pdf_inspector::types::TextItem],
    page: u32,
) -> Vec<OcrBlock> {
    let page_items: Vec<&pdf_inspector::types::TextItem> =
        items.iter().filter(|i| i.page == page).collect();
    if page_items.is_empty() {
        return Vec::new();
    }

    let page_height = page_items
        .iter()
        .map(|i| i.y + i.height)
        .fold(0.0f32, f32::max)
        .max(1.0);

    let mut blocks: Vec<OcrBlock> = page_items
        .into_iter()
        .filter(|i| !i.text.trim().is_empty())
        .map(|i| {
            let top = page_height - (i.y + i.height);
            let x_min = i.x;
            let y_min = top.max(0.0);
            let x_max = i.x + i.width.max(1.0);
            let y_max = (top + i.height).max(y_min + 1.0);
            OcrBlock {
                block_type: "text".to_string(),
                text: i.text.trim().to_string(),
                confidence: 1.0,
                bbox: [x_min, y_min, x_max, y_max],
                page,
                order: 0,
            }
        })
        .collect();

    // Reading order: top-to-bottom, then left-to-right.
    blocks.sort_by(|a, b| {
        a.bbox[1]
            .partial_cmp(&b.bbox[1])
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| {
                a.bbox[0]
                    .partial_cmp(&b.bbox[0])
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    });
    for (idx, b) in blocks.iter_mut().enumerate() {
        b.order = (idx + 1) as u32;
    }
    blocks
}
