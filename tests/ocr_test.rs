//! Tests for the OCR post-processing pipeline (T10).
//!
//! These unit tests cover the structured intermediate layer without requiring the
//! PP-OCRv6 models: synthetic `OcrBlock`s are fed through `process_ocr_result`,
//! reading-order, classification, paragraph merging, header/footer filtering, and
//! the `Document` → markdown renderer.

use omnid_lib::models::document::Block;
use omnid_lib::models::ocr::OcrBlock;
use omnid_lib::ocr::postprocess::{
    blocks_to_document, process_ocr_result, render_document, reorder_blocks,
};

fn blk(text: &str, bbox: [f32; 4], page: u32, order: u32) -> OcrBlock {
    OcrBlock {
        block_type: "text".to_string(),
        text: text.to_string(),
        confidence: 0.9,
        bbox,
        page,
        order,
    }
}

#[test]
fn test_ocr_mode_default_auto() {
    use omnid_lib::models::ocr::OcrMode;
    assert_eq!(OcrMode::default(), OcrMode::Auto);
}

#[test]
fn test_reorder_single_column_top_to_bottom() {
    let blocks = vec![
        blk("bottom line", [0.0, 100.0, 50.0, 120.0], 1, 1),
        blk("top line", [0.0, 0.0, 50.0, 20.0], 1, 2),
    ];
    let ordered = reorder_blocks(&blocks);
    assert_eq!(ordered[0].text, "top line");
    assert_eq!(ordered[1].text, "bottom line");
}

#[test]
fn test_reorder_double_column_left_then_right() {
    let blocks = vec![
        blk("L1", [0.0, 0.0, 40.0, 20.0], 1, 1),
        blk("L2", [0.0, 30.0, 40.0, 50.0], 1, 2),
        blk("L3", [0.0, 60.0, 40.0, 80.0], 1, 3),
        blk("R1", [200.0, 0.0, 240.0, 20.0], 1, 4),
        blk("R2", [200.0, 30.0, 240.0, 50.0], 1, 5),
        blk("R3", [200.0, 60.0, 240.0, 80.0], 1, 6),
    ];
    let ordered = reorder_blocks(&blocks);
    // Left column fully precedes the right column.
    assert_eq!(ordered[0].text, "L1");
    assert_eq!(ordered[2].text, "L3");
    assert_eq!(ordered[3].text, "R1");
}

#[test]
fn test_classify_title_near_top_is_heading() {
    // Title line is taller than the body lines; body lines must stay paragraphs.
    let blocks = vec![
        blk("Chapter One", [0.0, 10.0, 300.0, 50.0], 1, 1),
        blk("body line one text", [0.0, 200.0, 300.0, 220.0], 1, 2),
        blk("body line two text", [0.0, 240.0, 300.0, 260.0], 1, 3),
    ];
    let out = process_ocr_result(&blocks);
    assert!(matches!(out.first(), Some(Block::Heading { .. })));
    let headings = out.iter().filter(|b| matches!(b, Block::Heading { .. })).count();
    assert_eq!(headings, 1);
    // Body lines remain paragraphs, not headings.
    let headings = out.iter().filter(|b| matches!(b, Block::Heading { .. })).count();
    assert_eq!(headings, 1);
}

#[test]
fn test_merge_paragraphs_groups_lines() {
    let blocks = vec![
        blk("Hello world this is", [0.0, 0.0, 100.0, 20.0], 1, 1),
        blk("a continued paragraph", [0.0, 25.0, 100.0, 45.0], 1, 2),
        blk("separate paragraph", [0.0, 120.0, 100.0, 140.0], 1, 3),
    ];
    let out = process_ocr_result(&blocks);
    let paragraphs: Vec<&Block> = out.iter().filter(|b| matches!(b, Block::Paragraph { .. })).collect();
    assert_eq!(paragraphs.len(), 2, "two paragraphs expected");
}

#[test]
fn test_page_numbers_are_filtered() {
    let blocks = vec![
        blk("1", [400.0, 780.0, 410.0, 795.0], 1, 1),
        blk("2", [400.0, 780.0, 410.0, 795.0], 2, 1),
        blk("body text on page one", [0.0, 100.0, 100.0, 120.0], 1, 2),
    ];
    let out = process_ocr_result(&blocks);
    let joined = render_document(&out);
    assert!(!joined.contains("1\n2"));
    assert!(joined.contains("body text on page one"));
}

#[test]
fn test_repeated_header_filtered_across_pages() {
    // Same small text at the top band on every page → treated as a header.
    let blocks = vec![
        blk("Chapter Header", [10.0, 5.0, 200.0, 25.0], 1, 1),
        blk("page one body", [0.0, 100.0, 100.0, 120.0], 1, 2),
        blk("Chapter Header", [10.0, 5.0, 200.0, 25.0], 2, 1),
        blk("page two body", [0.0, 100.0, 100.0, 120.0], 2, 2),
    ];
    let out = process_ocr_result(&blocks);
    let joined = render_document(&out);
    assert!(!joined.contains("Chapter Header"));
    assert!(joined.contains("page one body"));
    assert!(joined.contains("page two body"));
}

#[test]
fn test_ocr_result_to_document_blocks() {
    let blocks = vec![
        blk("Title", [0.0, 5.0, 200.0, 30.0], 1, 1),
        blk("some body text here", [0.0, 100.0, 200.0, 120.0], 1, 2),
    ];
    let doc = blocks_to_document(process_ocr_result(&blocks), "doc.pdf", "ocr", 1234);
    assert_eq!(doc.metadata.format, "ocr");
    assert_eq!(doc.metadata.size_bytes, 1234);
    assert_eq!(doc.blocks.len(), 2);
}

#[test]
fn test_render_table_roundtrip() {
    let blocks = vec![
        blk("Name", [0.0, 0.0, 50.0, 20.0], 1, 1),
        blk("Age", [100.0, 0.0, 150.0, 20.0], 1, 2),
        blk("Alice", [0.0, 30.0, 50.0, 50.0], 1, 3),
        blk("30", [100.0, 30.0, 150.0, 50.0], 1, 4),
    ];
    let out = process_ocr_result(&blocks);
    let has_table = out.iter().any(|b| matches!(b, Block::Table { .. }));
    assert!(has_table, "aligned 2x2 grid should be detected as a table");
    let rendered = render_document(&out);
    assert!(rendered.contains("|"));
    assert!(rendered.contains("Name"));
}
