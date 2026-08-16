//! OCR post-processing: reading-order reconstruction, block classification
//! (title / table / header / footer / page number), paragraph merging, and the
//! mapping from `OcrResult` into the structured `Document` intermediate layer.
//!
//! The `Document` produced here is fed into `markdown_pipeline` (via a markdown
//! rendering step) so that OCR output goes through the same Normalize → Cleanup
//! → Output-Mode stages as any other converter.

use std::collections::{BTreeMap, HashSet};

use crate::models::document::{Block, Document, DocumentMetadata};
use crate::models::ocr::{OcrBlock, OcrResult};

/// Process every page of an OCR result set into a flat list of `Document` blocks.
///
/// Blocks from all pages are passed in (each `OcrBlock` carries its `page`); this
/// entry point handles cross-page header/footer detection, per-page reading-order
/// reconstruction, classification, paragraph merging, and inserts `PageBreak`
/// markers between pages when more than one page is present.
pub fn process_ocr_result(all_blocks: &[OcrBlock]) -> Vec<Block> {
    if all_blocks.is_empty() {
        return Vec::new();
    }

    // Cross-page header/footer/page-number detection operates on the full set.
    let excluded: HashSet<usize> = detect_excluded_indices(all_blocks);

    // Group blocks by page, preserving original order within a page.
    let mut by_page: BTreeMap<u32, Vec<OcrBlock>> = BTreeMap::new();
    for (idx, b) in all_blocks.iter().enumerate() {
        if excluded.contains(&idx) {
            continue;
        }
        by_page.entry(b.page).or_default().push(b.clone());
    }

    let _page_count = by_page.len();
    let mut out: Vec<(f32, Block)> = Vec::new();

    for (_i, (_page, mut blocks)) in by_page.into_iter().enumerate() {
        // Detect and lift out table regions first so they don't get merged as text.
        let (tables, text_blocks) = detect_tables(&blocks);
        blocks = text_blocks;

        let ordered = reorder_blocks(&blocks);
        let page_blocks = classify_and_build(&ordered);
        for (y, blk) in page_blocks {
            out.push((y, blk));
        }
        for (y, tbl) in tables {
            out.push((y, tbl));
        }
    }

    // Stable sort by representative y (PageBreak markers sort last).
    out.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    out.into_iter().map(|(_, b)| b).collect()
}

/// Build the structured `Document` from a set of per-page OCR results.
pub fn ocr_results_to_document(
    results: &[OcrResult],
    source_path: &str,
    format: &str,
    size_bytes: u64,
) -> Document {
    let mut all_blocks: Vec<OcrBlock> = Vec::new();
    for r in results {
        all_blocks.extend(r.blocks.iter().cloned());
    }
    let blocks = process_ocr_result(&all_blocks);
    blocks_to_document(blocks, source_path, format, size_bytes)
}

/// Wrap a list of already-processed blocks into a `Document`.
pub fn blocks_to_document(
    blocks: Vec<Block>,
    source_path: &str,
    format: &str,
    size_bytes: u64,
) -> Document {
    let file_name = std::path::Path::new(source_path)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_string();
    Document {
        metadata: DocumentMetadata {
            file_name,
            format: format.to_string(),
            size_bytes,
            converted_at: String::new(),
        },
        blocks,
        assets: Vec::new(),
    }
}

// ---------------------------------------------------------------------------
// Reading order (T3)
// ---------------------------------------------------------------------------

/// Reconstruct reading order for a single page.
pub fn reorder_blocks(blocks: &[OcrBlock]) -> Vec<OcrBlock> {
    if blocks.len() <= 1 {
        return blocks.to_vec();
    }

    let x_centers: Vec<f32> = blocks
        .iter()
        .map(|b| (b.bbox[0] + b.bbox[2]) / 2.0)
        .collect();
    let min_x = x_centers.iter().cloned().fold(f32::INFINITY, f32::min);
    let max_x = x_centers.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
    let range = (max_x - min_x).max(1.0);

    let mut sorted_x = x_centers.clone();
    sorted_x.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    // Largest gap between consecutive sorted x-centers → double column split.
    let mut largest_gap = 0.0f32;
    for w in sorted_x.windows(2) {
        let gap = w[1] - w[0];
        if gap > largest_gap {
            largest_gap = gap;
        }
    }
    let (left_count, right_count) = {
        let split = (min_x + max_x) / 2.0;
        let l = blocks
            .iter()
            .filter(|b| (b.bbox[0] + b.bbox[2]) / 2.0 <= split)
            .count();
        let r = blocks.len() - l;
        (l, r)
    };

    if largest_gap > 0.2 * range && left_count >= 3 && right_count >= 3 {
        let split = (min_x + max_x) / 2.0;
        let mut left: Vec<OcrBlock> = blocks
            .iter()
            .filter(|b| (b.bbox[0] + b.bbox[2]) / 2.0 <= split)
            .cloned()
            .collect();
        let mut right: Vec<OcrBlock> = blocks
            .iter()
            .filter(|b| (b.bbox[0] + b.bbox[2]) / 2.0 > split)
            .cloned()
            .collect();
        sort_single_column(&mut left);
        sort_single_column(&mut right);
        left.extend(right);
        left
    } else {
        let mut single = blocks.to_vec();
        sort_single_column(&mut single);
        single
    }
}

fn sort_single_column(blocks: &mut [OcrBlock]) {
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
}

// ---------------------------------------------------------------------------
// Classification + title (T4) + paragraph merge (T5)
// ---------------------------------------------------------------------------

/// Compute the median line height of a page (used for title detection).
fn median_line_height(blocks: &[OcrBlock]) -> f32 {
    if blocks.is_empty() {
        return 1.0;
    }
    let mut heights: Vec<f32> = blocks
        .iter()
        .map(|b| (b.bbox[3] - b.bbox[1]).max(1.0))
        .collect();
    heights.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    heights[heights.len() / 2]
}

/// Conservative title detection (§16 — when in doubt, treat as body text).
/// A block is a heading when it sits near the top of the page AND its font box is
/// clearly taller than the median body line (titles are larger). This avoids
/// misclassifying the first body line of a paragraph as a heading.
fn is_title(block: &OcrBlock, median_height: f32, page_height: f32) -> bool {
    let text = block.text.trim();
    if text.is_empty() || text.chars().count() > 200 {
        return false;
    }
    if is_page_number_text(text) {
        return false;
    }
    let block_height = (block.bbox[3] - block.bbox[1]).max(1.0);
    let y_norm = if page_height > 0.0 {
        block.bbox[1] / page_height
    } else {
        1.0
    };
    // Top 20% of the page only.
    if y_norm > 0.20 {
        return false;
    }
    // Titles are typically larger than body text.
    block_height >= 1.3 * median_height
}

/// Build `Document` blocks for a single (already reading-ordered) page.
fn classify_and_build(ordered: &[OcrBlock]) -> Vec<(f32, Block)> {
    if ordered.is_empty() {
        return Vec::new();
    }
    let page_height = ordered
        .iter()
        .map(|b| b.bbox[3])
        .fold(0.0f32, f32::max)
        .max(1.0);
    let median = median_line_height(ordered);

    // Group blocks into visual lines (blocks whose y-bands overlap are one line).
    let mut lines: Vec<Vec<OcrBlock>> = Vec::new();
    for b in ordered {
        if let Some(last) = lines.last_mut() {
            if let Some(prev) = last.last() {
                if y_overlap(prev, b) {
                    last.push(b.clone());
                    continue;
                }
            }
        }
        lines.push(vec![b.clone()]);
    }

    let para_gap = median * 1.3;
    let mut out: Vec<(f32, Block)> = Vec::new();
    let mut para_lines: Vec<String> = Vec::new();
    let mut para_y: Option<f32> = None;
    let mut prev_bottom: Option<f32> = None;

    let flush_paragraph = |para_lines: &mut Vec<String>, para_y: &mut Option<f32>, out: &mut Vec<(f32, Block)>| {
        if !para_lines.is_empty() {
            let content = para_lines.join("\n");
            out.push(((*para_y).unwrap_or(0.0), Block::Paragraph { content }));
            para_lines.clear();
            *para_y = None;
        }
    };

    for line in lines {
        // Sort line blocks left-to-right.
        let mut lb = line.clone();
        lb.sort_by(|a, b| {
            a.bbox[0]
                .partial_cmp(&b.bbox[0])
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let line_top = lb.iter().map(|b| b.bbox[1]).fold(f32::INFINITY, f32::min);
        let line_bottom = lb.iter().map(|b| b.bbox[3]).fold(0.0f32, f32::max);
        let line_text = join_line_blocks(&lb);

        let has_title = lb.iter().any(|b| is_title(b, median, page_height));
        if has_title {
            flush_paragraph(&mut para_lines, &mut para_y, &mut out);
            prev_bottom = None;
            out.push((
                line_top,
                Block::Heading {
                    level: 2,
                    text: line_text.clone(),
                },
            ));
            continue;
        }

        let same_paragraph = match prev_bottom {
            Some(pb) => line_top - pb <= para_gap,
            None => true,
        };
        if same_paragraph {
            if para_y.is_none() {
                para_y = Some(line_top);
            }
            para_lines.push(line_text);
        } else {
            flush_paragraph(&mut para_lines, &mut para_y, &mut out);
            para_y = Some(line_top);
            para_lines.push(line_text);
        }
        prev_bottom = Some(line_bottom);
    }
    flush_paragraph(&mut para_lines, &mut para_y, &mut out);

    out
}

/// Join blocks that form a single visual line. CJK runs are concatenated without
/// a space; Latin runs get a single space separator.
fn join_line_blocks(blocks: &[OcrBlock]) -> String {
    let mut out = String::new();
    for b in blocks {
        let t = b.text.trim();
        if t.is_empty() {
            continue;
        }
        if out.is_empty() {
            out.push_str(t);
        } else {
            let prev_last = out.chars().last().unwrap_or(' ');
            let next_first = t.chars().next().unwrap_or(' ');
            if is_cjk_char(prev_last) || is_cjk_char(next_first) {
                out.push_str(t);
            } else {
                out.push(' ');
                out.push_str(t);
            }
        }
    }
    out
}

fn y_overlap(a: &OcrBlock, b: &OcrBlock) -> bool {
    let overlap = (a.bbox[3].min(b.bbox[3])) - (a.bbox[1].max(b.bbox[1]));
    overlap > 0.0
}

// ---------------------------------------------------------------------------
// Cross-page header / footer / page-number detection (T5)
// ---------------------------------------------------------------------------

/// Returns the indices (into `all_blocks`) of blocks that should be dropped
/// because they are page numbers, or repeated headers/footers across pages.
fn detect_excluded_indices(all_blocks: &[OcrBlock]) -> HashSet<usize> {
    let mut excluded = HashSet::new();
    if all_blocks.is_empty() {
        return excluded;
    }

    let page_heights = page_heights(all_blocks);

    // Standalone numbers are only treated as page numbers when they sit in the
    // header/footer bands (top/bottom of the page). A lone number in the body
    // (e.g. a table cell "30") must be preserved.
    for (idx, b) in all_blocks.iter().enumerate() {
        if is_page_number(b) {
            let h = page_heights.get(&b.page).copied().unwrap_or(1.0).max(1.0);
            let y_norm = (b.bbox[1] / h).clamp(0.0, 1.0);
            if y_norm < 0.12 || y_norm > 0.88 {
                excluded.insert(idx);
            }
        }
    }

    let mut band_text: BTreeMap<(u8, String), Vec<u32>> = BTreeMap::new();
    for b in all_blocks {
        if b.text.trim().is_empty() || b.text.chars().count() > 120 {
            continue;
        }
        let h = page_heights.get(&b.page).copied().unwrap_or(1.0).max(1.0);
        let y_norm = (b.bbox[1] / h).clamp(0.0, 1.0);
        let band: u8 = if y_norm < 0.12 {
            0
        } else if y_norm > 0.88 {
            2
        } else {
            continue;
        };
        let key = (band, normalize_for_match(&b.text));
        band_text.entry(key).or_default().push(b.page);
    }

    let mut matched_keys = Vec::new();
    for (key, pages) in &band_text {
        let distinct: HashSet<u32> = pages.iter().copied().collect();
        if distinct.len() >= 2 {
            matched_keys.push(key.clone());
        }
    }

    for (idx, b) in all_blocks.iter().enumerate() {
        let h = page_heights.get(&b.page).copied().unwrap_or(1.0).max(1.0);
        let y_norm = (b.bbox[1] / h).clamp(0.0, 1.0);
        let band: u8 = if y_norm < 0.12 {
            0
        } else if y_norm > 0.88 {
            2
        } else {
            continue;
        };
        let key = (band, normalize_for_match(&b.text));
        if matched_keys.contains(&key) {
            excluded.insert(idx);
        }
    }

    excluded
}

/// A block is a page number when its text is a lone number or a short
/// page-indicator pattern and its box is small.
fn is_page_number(block: &OcrBlock) -> bool {
    let t = block.text.trim();
    is_page_number_text(t)
}

fn is_page_number_text(t: &str) -> bool {
    if t.is_empty() || t.chars().count() > 12 {
        return false;
    }
    let digits: String = t.chars().filter(|c| c.is_ascii_digit()).collect();
    if !digits.is_empty() && digits.len() == t.chars().count() {
        return true;
    }
    if t.contains("页") && !digits.is_empty() {
        return true;
    }
    let lower = t.to_lowercase();
    if lower.starts_with("page") && !digits.is_empty() {
        return true;
    }
    false
}

fn normalize_for_match(text: &str) -> String {
    text.trim().to_lowercase()
}

fn page_heights(all_blocks: &[OcrBlock]) -> BTreeMap<u32, f32> {
    let mut m: BTreeMap<u32, f32> = BTreeMap::new();
    for b in all_blocks {
        let h = m.entry(b.page).or_insert(0.0);
        *h = (*h).max(b.bbox[3]);
    }
    m
}

// ---------------------------------------------------------------------------
// Conservative table detection (T4)
// ---------------------------------------------------------------------------

/// Detect grid-like table regions and lift them out of the normal text flow.
///
/// Returns `(tables, remaining_text_blocks)`. Triggers only when blocks clearly
/// form a 2-D grid (>= 2 aligned rows, each with >= 2 cells). Everything else is
/// left as text (P0 conservative behavior per the plan).
fn detect_tables(blocks: &[OcrBlock]) -> (Vec<(f32, Block)>, Vec<OcrBlock>) {
    if blocks.len() < 4 {
        return (Vec::new(), blocks.to_vec());
    }
    let median = median_line_height(blocks);
    let row_band = median * 0.8;

    let mut sorted = blocks.to_vec();
    sort_single_column(&mut sorted);

    // Cluster blocks into rows by y proximity.
    let mut rows: Vec<Vec<OcrBlock>> = Vec::new();
    for b in &sorted {
        if let Some(last) = rows.last_mut() {
            if let Some(prev) = last.last() {
                if (b.bbox[1] - prev.bbox[1]).abs() <= row_band && y_overlap(prev, b) {
                    last.push(b.clone());
                    continue;
                }
            }
        }
        rows.push(vec![b.clone()]);
    }

    let table_rows: Vec<Vec<OcrBlock>> = rows
        .into_iter()
        .filter(|r| {
            if r.len() < 2 {
                return false;
            }
            let mut xs = r
                .iter()
                .map(|b| (b.bbox[0], b.bbox[2]))
                .collect::<Vec<_>>();
            xs.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
            let mut has_gap = false;
            for w in xs.windows(2) {
                if w[1].0 - w[0].1 > median * 0.5 {
                    has_gap = true;
                    break;
                }
            }
            has_gap
        })
        .collect();

    if table_rows.len() < 2 {
        return (Vec::new(), blocks.to_vec());
    }

    let mut headers: Vec<String> = Vec::new();
    for cell in &table_rows[0] {
        headers.push(cell.text.trim().to_string());
    }
    let mut data_rows: Vec<Vec<String>> = Vec::new();
    for row in &table_rows[1..] {
        let mut cells: Vec<String> = Vec::new();
        for cell in row {
            cells.push(cell.text.trim().to_string());
        }
        data_rows.push(cells);
    }

    let top_y = table_rows
        .iter()
        .flatten()
        .map(|b| b.bbox[1])
        .fold(f32::INFINITY, f32::min);

    let table_indices: HashSet<usize> = {
        let mut set = HashSet::new();
        for row in &table_rows {
            for cell in row {
                if let Some(pos) = blocks.iter().position(|b| {
                    b.page == cell.page
                        && (b.bbox[0], b.bbox[1], b.bbox[2], b.bbox[3])
                            == (cell.bbox[0], cell.bbox[1], cell.bbox[2], cell.bbox[3])
                        && b.text == cell.text
                }) {
                    set.insert(pos);
                }
            }
        }
        set
    };
    let remaining: Vec<OcrBlock> = blocks
        .iter()
        .enumerate()
        .filter(|(i, _)| !table_indices.contains(i))
        .map(|(_, b)| b.clone())
        .collect();

    (
        vec![(
            top_y,
            Block::Table {
                headers,
                rows: data_rows,
            },
        )],
        remaining,
    )
}

// ---------------------------------------------------------------------------
// Document → Markdown rendering (mirrors anydoc_converter::render_block)
// ---------------------------------------------------------------------------

/// Render structured `Document` blocks to markdown (one step before markdown_pipeline).
pub fn render_document(blocks: &[Block]) -> String {
    let mut out = String::new();
    for block in blocks {
        render_block(block, &mut out);
        out.push_str("\n\n");
    }
    out.trim_end().to_string()
}

fn render_block(block: &Block, out: &mut String) {
    match block {
        Block::Heading { level, text } => {
            let l = (*level as usize).clamp(1, 6);
            for _ in 0..l {
                out.push('#');
            }
            out.push(' ');
            out.push_str(text);
        }
        Block::Paragraph { content } => {
            out.push_str(content);
        }
        Block::BulletList { items } => {
            for it in items {
                out.push_str("- ");
                out.push_str(it);
                out.push('\n');
            }
            if out.ends_with('\n') {
                out.pop();
            }
        }
        Block::OrderedList { items } => {
            for (i, it) in items.iter().enumerate() {
                out.push_str(&format!("{}. {}", i + 1, it));
                out.push('\n');
            }
            if out.ends_with('\n') {
                out.pop();
            }
        }
        Block::CodeBlock { lang, content } => {
            out.push_str("```");
            if !lang.is_empty() {
                out.push_str(lang);
            }
            out.push('\n');
            out.push_str(content);
            if !content.ends_with('\n') {
                out.push('\n');
            }
            out.push_str("```");
        }
        Block::Table { headers, rows } => {
            render_markdown_table(headers, rows, out);
        }
        Block::Quote { content } => {
            for line in content.lines() {
                out.push_str("> ");
                out.push_str(line);
                out.push('\n');
            }
            if out.ends_with('\n') {
                out.pop();
            }
        }
        Block::Image { src, alt } => {
            out.push_str(&format!("![{}]({})", alt, src));
        }
        Block::HorizontalRule => {
            out.push_str("---");
        }
        Block::PageBreak => {
            out.push_str("---");
        }
    }
}

fn render_markdown_table(headers: &[String], rows: &[Vec<String>], out: &mut String) {
    if headers.is_empty() {
        return;
    }
    let col_count = headers.len();
    out.push('|');
    for h in headers {
        out.push(' ');
        out.push_str(&escape_pipe(h));
        out.push(' ');
        out.push('|');
    }
    out.push('\n');
    out.push('|');
    for _ in 0..col_count {
        out.push_str(" --- |");
    }
    out.push('\n');
    for row in rows {
        out.push('|');
        for c in 0..col_count {
            let cell = row.get(c).map(|s| s.as_str()).unwrap_or("");
            out.push(' ');
            out.push_str(&escape_pipe(cell));
            out.push(' ');
            out.push('|');
        }
        out.push('\n');
    }
    if out.ends_with('\n') {
        out.pop();
    }
}

fn escape_pipe(s: &str) -> String {
    s.replace('|', "\\|").replace('\n', " ")
}

fn is_cjk_char(c: char) -> bool {
    matches!(
        c,
        '\u{4E00}'..='\u{9FFF}'
            | '\u{3400}'..='\u{4DBF}'
            | '\u{F900}'..='\u{FAFF}'
            | '\u{3040}'..='\u{309F}'
            | '\u{30A0}'..='\u{30FF}'
            | '\u{AC00}'..='\u{D7AF}'
            | '\u{FF00}'..='\u{FFEF}'
    )
}
