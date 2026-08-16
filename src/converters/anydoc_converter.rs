use crate::models::{
    document::{Asset, Block, Document},
    task::{ConversionError, ConversionResult, ConversionStage, ErrorCode},
    Converter,
};

pub struct AnyDocConverter;

const SUPPORTED_FORMATS: &[&str] = &[
    "pdf", "docm", "docx", "doc", "pptx", "pptm", "ppsx", "ppsm", "ppt", "pps", "pot",
    "xlsb", "xlsm", "xlsx", "xls", "epub", "csv", "txt", "html", "htm", "odt", "ods", "odp",
    "rtf",
];

const TEXT_PASSTHROUGH_FORMATS: &[&str] = &["txt", "json", "xml", "html", "htm"];

impl AnyDocConverter {
    pub fn new() -> Self {
        AnyDocConverter
    }

    fn map_error(err: &anydoc::ConvertError) -> ConversionError {
        let (code, retryable) = match err {
            anydoc::ConvertError::Unsupported(_) => (ErrorCode::Unsupported, false),
            anydoc::ConvertError::Encrypted => (ErrorCode::Encrypted, false),
            anydoc::ConvertError::Malformed { .. } => (ErrorCode::Malformed, false),
            anydoc::ConvertError::ResourceLimit { .. } => (ErrorCode::IoError, false),
            anydoc::ConvertError::MissingPart { .. } => (ErrorCode::Malformed, false),
            anydoc::ConvertError::Io(_) => (ErrorCode::IoError, true),
            &_ => (ErrorCode::Malformed, false),
        };

        ConversionError {
            code,
            message: err.to_string(),
            stage: ConversionStage::Extracting,
            retryable,
            page: None,
        }
    }

    fn convert_text_passthrough(&self, bytes: &[u8], format: &str) -> ConversionResult {
        let content = String::from_utf8_lossy(bytes).to_string();
        let mut blocks = Vec::new();

        if format == "json" || format == "xml" {
            blocks.push(Block::CodeBlock {
                lang: format.to_string(),
                content: content.clone(),
            });
        } else {
            blocks.push(Block::Paragraph {
                content: content.clone(),
            });
        }

        let doc = Document {
            metadata: crate::models::document::DocumentMetadata {
                file_name: String::new(),
                format: format.to_string(),
                size_bytes: bytes.len() as u64,
                converted_at: String::new(),
            },
            blocks,
            assets: Vec::new(),
        };

        ConversionResult {
            task_id: String::new(),
            markdown: content,
            document: doc,
            assets: Vec::new(),
            errors: Vec::new(),
            output_path: String::new(),
            stats: None,
        }
    }

    // -----------------------------------------------------------------------
    // Markdown rendering from the anydoc Document model.
    // This is the core quality improvement: we render images, links, and
    // styled text properly instead of using anydoc's built-in renderer which
    // drops embedded images as plain alt text.
    // -----------------------------------------------------------------------

    /// Render an anydoc Document to a markdown string, properly handling
    /// embedded images (as `![alt](assets/filename.ext)`), hyperlinks, and
    /// character styles (bold, italic, code, strikethrough).
    fn render_markdown(doc: &anydoc::model::Document, assets: &[Asset]) -> String {
        let mut out = String::new();
        for block in &doc.blocks {
            Self::render_block(block, assets, &mut out);
            out.push_str("\n\n");
        }
        // Trim trailing newlines.
        out.trim_end().to_string()
    }

    fn render_block(block: &anydoc::model::Block, assets: &[Asset], out: &mut String) {
        match block {
            anydoc::model::Block::Heading { level, content, .. } => {
                let l = (*level as usize).min(6);
                for _ in 0..l {
                    out.push('#');
                }
                out.push(' ');
                Self::render_inlines(content, assets, out);
            }
            anydoc::model::Block::Paragraph(inlines) => {
                Self::render_inlines(inlines, assets, out);
            }
            anydoc::model::Block::List(list) => {
                Self::render_list(list, assets, out);
            }
            anydoc::model::Block::Table(table) => {
                Self::render_table(table, assets, out);
            }
            anydoc::model::Block::BlockQuote(blocks) => {
                let mut inner = String::new();
                for b in blocks {
                    Self::render_block(b, assets, &mut inner);
                    inner.push_str("\n\n");
                }
                let inner = inner.trim_end();
                for line in inner.lines() {
                    out.push_str("> ");
                    out.push_str(line);
                    out.push('\n');
                }
                // Remove trailing newline added by the loop.
                if out.ends_with('\n') {
                    out.pop();
                }
            }
            anydoc::model::Block::CodeBlock { lang, text } => {
                out.push_str("```");
                if let Some(l) = lang {
                    out.push_str(l);
                }
                out.push('\n');
                out.push_str(text);
                if !text.ends_with('\n') {
                    out.push('\n');
                }
                out.push_str("```");
            }
            anydoc::model::Block::Rule => {
                out.push_str("---");
            }
        }
    }

    fn render_inlines(inlines: &[anydoc::model::Inline], assets: &[Asset], out: &mut String) {
        for inline in inlines {
            match inline {
                anydoc::model::Inline::Text { text, style } => {
                    Self::render_styled_text(text, *style, out);
                }
                anydoc::model::Inline::Link { content, target } => {
                    let mut label = String::new();
                    Self::render_inlines(content, assets, &mut label);
                    let url = match target {
                        anydoc::model::LinkTarget::External(u) => u.clone(),
                        anydoc::model::LinkTarget::Relative(u) => u.clone(),
                        anydoc::model::LinkTarget::Anchor(_) => String::new(),
                    };
                    if !url.is_empty() {
                        out.push_str(&format!("[{}]({})", label.trim(), url));
                    } else if !label.is_empty() {
                        out.push_str(&label);
                    }
                }
                anydoc::model::Inline::Image { alt, source } => {
                    match source {
                        anydoc::model::ImageSource::External(url) => {
                            let alt_clean = alt.trim();
                            out.push_str(&format!("![{}]({})", alt_clean, url));
                        }
                        anydoc::model::ImageSource::Asset(id) => {
                            if let Some(asset) = assets.get(id.0) {
                                let alt_clean = if alt.trim().is_empty() {
                                    "image"
                                } else {
                                    alt.trim()
                                };
                                out.push_str(&format!(
                                    "![{}](assets/{})",
                                    alt_clean, asset.name
                                ));
                            } else if !alt.trim().is_empty() {
                                out.push_str(alt.trim());
                            }
                        }
                        anydoc::model::ImageSource::Unavailable => {
                            if !alt.trim().is_empty() {
                                out.push_str(alt.trim());
                            }
                        }
                    }
                }
                anydoc::model::Inline::Anchor(_) => {}
                anydoc::model::Inline::NoteRef(id) => {
                    out.push_str(&format!("[^{}]", id));
                }
                anydoc::model::Inline::LineBreak => {
                    out.push_str("  \n");
                }
            }
        }
    }

    /// Render styled text with markdown emphasis markers.
    fn render_styled_text(text: &str, style: anydoc::model::Style, out: &mut String) {
        if style == anydoc::model::Style::PLAIN {
            out.push_str(text);
            return;
        }

        // Split leading/trailing whitespace from the core text so emphasis
        // markers wrap only the non-whitespace portion.
        let core_start = text.len() - text.trim_start().len();
        let core_end = text.trim_end().len();
        let (lead, core, trail) = {
            if core_start >= core_end {
                // All whitespace.
                out.push_str(text);
                return;
            }
            (
                &text[..core_start],
                &text[core_start..core_end],
                &text[core_end..],
            )
        };

        if !lead.is_empty() {
            out.push_str(lead);
        }

        if !core.is_empty() {
            if style.code {
                out.push('`');
                out.push_str(core);
                out.push('`');
            } else {
                let mut open = String::new();
                if style.strike {
                    open.push_str("~~");
                }
                if style.bold {
                    open.push_str("**");
                }
                if style.italic {
                    open.push('*');
                }
                let close: String = open.chars().rev().collect();
                out.push_str(&open);
                out.push_str(core);
                out.push_str(&close);
            }
        }

        if !trail.is_empty() {
            out.push_str(trail);
        }
    }

    fn render_list(list: &anydoc::model::List, assets: &[Asset], out: &mut String) {
        let ordered = list.marker.ordered();
        for (i, item) in list.items.iter().enumerate() {
            let n = if ordered {
                list.start + i as u64
            } else {
                0
            };
            let marker = list.marker.label(n);
            out.push_str(&marker);
            out.push(' ');

            // Render item content.
            let mut content = String::new();
            for b in &item.blocks {
                Self::render_block(b, assets, &mut content);
                content.push_str("\n\n");
            }
            let content = content.trim_end();

            // For multi-line content, indent continuation lines.
            let mut first = true;
            for line in content.lines() {
                if first {
                    out.push_str(line);
                    out.push('\n');
                    first = false;
                } else {
                    out.push_str("  ");
                    out.push_str(line);
                    out.push('\n');
                }
            }
            if first {
                // Empty item.
                out.push('\n');
            }
        }
        // Remove trailing newline.
        if out.ends_with('\n') {
            out.pop();
        }
    }

    fn render_table(table: &anydoc::model::Table, assets: &[Asset], out: &mut String) {
        let header_count = table.header_rows;
        let grid = &table.grid;

        if grid.is_empty() {
            return;
        }

        // Determine column count (max row width).
        let col_count = grid.iter().map(|r| r.len()).max().unwrap_or(0);
        if col_count == 0 {
            return;
        }

        // Collect cell text into a 2D vector.
        let mut rows: Vec<Vec<String>> = Vec::new();
        for row in grid {
            let mut cells = Vec::new();
            for slot in row {
                let text = match slot {
                    anydoc::model::CellSlot::Origin(cell) => {
                        let mut s = String::new();
                        for b in &cell.blocks {
                            Self::render_block(b, assets, &mut s);
                        }
                        s.trim().replace('\n', " ").replace('|', "\\|")
                    }
                    anydoc::model::CellSlot::Covered { .. } => String::new(),
                };
                cells.push(text);
            }
            // Pad to col_count.
            while cells.len() < col_count {
                cells.push(String::new());
            }
            rows.push(cells);
        }

        if rows.is_empty() {
            return;
        }

        // Output the table.
        // Determine the header row and data rows.
        // If the table has explicit header rows, use the first one;
        // otherwise use the first data row as the header.
        let (header_row, data_start) = if header_count > 0 {
            (rows[0].clone(), header_count.min(rows.len()))
        } else {
            (rows[0].clone(), 1)
        };

        let data_rows: &[Vec<String>] = if data_start < rows.len() {
            &rows[data_start..]
        } else {
            &[]
        };

        // If no data rows and only one header row, render as plain text.
        if data_rows.is_empty() {
            for (i, cell) in header_row.iter().enumerate() {
                if i > 0 {
                    out.push_str(" | ");
                }
                out.push_str(cell);
            }
            return;
        }

        // Build header line.
        out.push_str("| ");
        for (i, cell) in header_row.iter().enumerate() {
            if i > 0 {
                out.push_str(" | ");
            }
            out.push_str(cell);
        }
        out.push_str(" |\n");

        // Separator line.
        out.push_str("| ");
        for i in 0..col_count {
            if i > 0 {
                out.push_str(" | ");
            }
            out.push_str("---");
        }
        out.push_str(" |\n");

        // Data rows.
        for row in data_rows {
            out.push_str("| ");
            for (i, cell) in row.iter().enumerate() {
                if i > 0 {
                    out.push_str(" | ");
                }
                out.push_str(cell);
            }
            out.push_str(" |\n");
        }

        // Remove trailing newline.
        if out.ends_with('\n') {
            out.pop();
        }
    }

    // -----------------------------------------------------------------------
    // Block mapping (for the structured Document model)
    // -----------------------------------------------------------------------

    fn flatten_blocks(blocks: Vec<anydoc::model::Block>) -> Vec<Block> {
        let mut result = Vec::new();
        for b in blocks {
            result.extend(Self::map_block(b));
        }
        result
    }

    fn map_block(b: anydoc::model::Block) -> Vec<Block> {
        match b {
            anydoc::model::Block::Heading { level, content, .. } => {
                let text = anydoc::model::inlines_to_plain_text(&content);
                vec![Block::Heading {
                    level: level.min(6),
                    text,
                }]
            }
            anydoc::model::Block::Paragraph(inlines) => {
                let content = anydoc::model::inlines_to_plain_text(&inlines);
                if content.trim().is_empty() {
                    Vec::new()
                } else {
                    vec![Block::Paragraph { content }]
                }
            }
            anydoc::model::Block::List(list) => {
                let items: Vec<String> = list
                    .items
                    .iter()
                    .map(|item| {
                        Self::flatten_blocks(item.blocks.clone())
                            .iter()
                            .map(|b| Self::block_to_string(b))
                            .collect::<Vec<_>>()
                            .join(" ")
                    })
                    .collect();

                if list.marker.ordered() {
                    vec![Block::OrderedList { items }]
                } else {
                    vec![Block::BulletList { items }]
                }
            }
            anydoc::model::Block::Table(table) => {
                let (headers, rows) = Self::table_to_vecs(&table);
                vec![Block::Table { headers, rows }]
            }
            anydoc::model::Block::BlockQuote(blocks) => {
                let content = Self::flatten_blocks(blocks)
                    .iter()
                    .map(|b| Self::block_to_string(b))
                    .collect::<Vec<_>>()
                    .join("\n");
                vec![Block::Quote { content }]
            }
            anydoc::model::Block::CodeBlock { lang, text } => vec![Block::CodeBlock {
                lang: lang.unwrap_or_default(),
                content: text,
            }],
            anydoc::model::Block::Rule => vec![Block::HorizontalRule],
        }
    }

    fn block_to_string(b: &Block) -> String {
        match b {
            Block::Heading { text, .. } => text.clone(),
            Block::Paragraph { content } => content.clone(),
            Block::BulletList { items } => items.join(" "),
            Block::OrderedList { items } => items.join(" "),
            Block::CodeBlock { content, .. } => content.clone(),
            Block::Table { rows, .. } => rows
                .iter()
                .map(|r| r.join(" | "))
                .collect::<Vec<_>>()
                .join("\n"),
            Block::Image { alt, .. } => alt.clone(),
            Block::Quote { content } => content.clone(),
            Block::HorizontalRule => String::new(),
            Block::PageBreak => String::new(),
        }
    }

    fn table_to_vecs(table: &anydoc::model::Table) -> (Vec<String>, Vec<Vec<String>>) {
        let mut headers = Vec::new();
        let mut rows = Vec::new();
        let header_count = table.header_rows;

        for (i, row) in table.grid.iter().enumerate() {
            let cells: Vec<String> = row
                .iter()
                .map(|slot| match slot {
                    anydoc::model::CellSlot::Origin(cell) => Self::cell_to_text(cell),
                    anydoc::model::CellSlot::Covered { .. } => String::new(),
                })
                .collect();

            if i < header_count {
                headers = cells;
            } else {
                rows.push(cells);
            }
        }

        if rows.is_empty() && !headers.is_empty() {
            rows = vec![headers.clone()];
            headers = Vec::new();
        }

        (headers, rows)
    }

    fn cell_to_text(cell: &anydoc::model::Cell) -> String {
        Self::flatten_blocks(cell.blocks.clone())
            .iter()
            .map(|b| Self::block_to_string(b))
            .collect::<Vec<_>>()
            .join(" ")
    }

    // -----------------------------------------------------------------------
    // Asset mapping — generate stable, predictable filenames.
    // -----------------------------------------------------------------------

    fn map_anydoc_assets(anydoc_assets: Vec<anydoc::model::Asset>) -> Vec<Asset> {
        anydoc_assets
            .into_iter()
            .enumerate()
            .map(|(i, asset)| {
                // Derive extension from media_type or origin_part.
                let ext = derive_extension(&asset.media_type, &asset.origin_part);

                // Stable name: image-001.png, image-002.jpeg, etc.
                let name = format!("image-{:03}.{}", i + 1, ext);

                Asset {
                    name,
                    extension: ext,
                    bytes: asset.bytes,
                    media_type: asset.media_type,
                }
            })
            .collect()
    }
}

/// Derive a file extension from MIME type or origin part name.
fn derive_extension(media_type: &str, origin_part: &str) -> String {
    // Try origin_part first — it's the most reliable source.
    if !origin_part.is_empty() {
        if let Some(ext) = std::path::Path::new(origin_part)
            .extension()
            .and_then(|e| e.to_str())
        {
            return ext.to_lowercase();
        }
    }

    // Fall back to media_type.
    match media_type {
        "image/png" => "png",
        "image/jpeg" | "image/jpg" => "jpg",
        "image/gif" => "gif",
        "image/bmp" => "bmp",
        "image/webp" => "webp",
        "image/svg+xml" => "svg",
        "image/tiff" | "image/tif" => "tiff",
        "image/x-icon" | "image/vnd.microsoft.icon" => "ico",
        "image/emf" => "emf",
        "image/wmf" => "wmf",
        _ => "png", // Safe default.
    }
    .to_string()
}

impl Converter for AnyDocConverter {
    fn name(&self) -> &str {
        "anydoc"
    }

    fn supports(&self, format: &str) -> bool {
        let fmt = format.to_lowercase();
        let trimmed = fmt.trim_start_matches('.');
        SUPPORTED_FORMATS.contains(&trimmed)
    }

    fn convert(&self, bytes: &[u8]) -> Result<ConversionResult, ConversionError> {
        let anydoc_format = anydoc::Format::from_bytes(bytes);

        let format_str = match &anydoc_format {
            Some(anydoc::Format::Doc) => "doc".to_string(),
            Some(anydoc::Format::Docx) => "docx".to_string(),
            Some(anydoc::Format::Odt) => "odt".to_string(),
            Some(anydoc::Format::Pdf) => "pdf".to_string(),
            Some(anydoc::Format::Ppt) => "ppt".to_string(),
            Some(anydoc::Format::Pptx) => "pptx".to_string(),
            Some(anydoc::Format::Rtf) => "rtf".to_string(),
            Some(anydoc::Format::Epub) => "epub".to_string(),
            Some(anydoc::Format::Excel) => "xlsx".to_string(),
            Some(anydoc::Format::Ods) => "ods".to_string(),
            Some(anydoc::Format::Odp) => "odp".to_string(),
            Some(anydoc::Format::Csv) => "csv".to_string(),
            None => {
                let raw = String::from_utf8_lossy(bytes).to_string();
                if raw.starts_with("<") || raw.starts_with("<!DOCTYPE") || raw.starts_with("<!doctype") {
                    return Ok(self.convert_text_passthrough(bytes, "html"));
                } else if raw.starts_with("{") {
                    return Ok(self.convert_text_passthrough(bytes, "json"));
                } else if raw.starts_with("<?xml") {
                    return Ok(self.convert_text_passthrough(bytes, "xml"));
                } else {
                    return Ok(self.convert_text_passthrough(bytes, "txt"));
                }
            }
        };

        if TEXT_PASSTHROUGH_FORMATS.contains(&format_str.as_str()) {
            return Ok(self.convert_text_passthrough(bytes, &format_str));
        }

        if anydoc_format == Some(anydoc::Format::Pdf) {
            match anydoc::to_markdown_bytes(bytes, anydoc::Format::Pdf) {
                Ok(markdown) => {
                    let doc = Document {
                        metadata: crate::models::document::DocumentMetadata {
                            file_name: String::new(),
                            format: format_str,
                            size_bytes: bytes.len() as u64,
                            converted_at: String::new(),
                        },
                        blocks: vec![Block::Paragraph {
                            content: markdown.clone(),
                        }],
                        assets: Vec::new(),
                    };
                    return Ok(ConversionResult {
                        task_id: String::new(),
                        markdown,
                        document: doc,
                        assets: Vec::new(),
                        errors: Vec::new(),
                        output_path: String::new(),
                        stats: None,
                    });
                }
                Err(e) => {
                    let err_msg = e.to_string();
                    if err_msg.contains("no extractable text") {
                        match pdf_inspector::extractor::extract_text_mem(bytes) {
                            Ok(text) => {
                                if text.trim().is_empty() {
                                    return Err(Self::map_error(&e));
                                }
                                let mut markdown = text.clone();
                                if !markdown.ends_with('\n') {
                                    markdown.push('\n');
                                }
                                let doc = Document {
                                    metadata: crate::models::document::DocumentMetadata {
                                        file_name: String::new(),
                                        format: format_str,
                                        size_bytes: bytes.len() as u64,
                                        converted_at: String::new(),
                                    },
                                    blocks: vec![Block::Paragraph {
                                        content: markdown.clone(),
                                    }],
                                    assets: Vec::new(),
                                };
                                return Ok(ConversionResult {
                                    task_id: String::new(),
                                    markdown,
                                    document: doc,
                                    assets: Vec::new(),
                                    errors: Vec::new(),
                                    output_path: String::new(),
                                    stats: None,
                                });
                            }
                            Err(fallback_err) => {
                                tracing::warn!("PDF fallback extraction also failed: {}", fallback_err);
                                return Err(Self::map_error(&e));
                            }
                        }
                    }
                    return Err(Self::map_error(&e));
                }
            }
        }

        match anydoc::to_document(bytes, anydoc_format) {
            Ok(anydoc_doc) => {
                let blocks = Self::flatten_blocks(anydoc_doc.blocks.clone());
                let assets = Self::map_anydoc_assets(anydoc_doc.assets.clone());

                // Build markdown using our custom renderer that properly
                // handles embedded images, links, and styled text.
                let markdown = Self::render_markdown(&anydoc_doc, &assets);

                let document = Document {
                    metadata: crate::models::document::DocumentMetadata {
                        file_name: String::new(),
                        format: format_str,
                        size_bytes: bytes.len() as u64,
                        converted_at: String::new(),
                    },
                    blocks,
                    assets: assets.clone(),
                };

                Ok(ConversionResult {
                    task_id: String::new(),
                    markdown,
                    document,
                    assets,
                    errors: Vec::new(),
                    output_path: String::new(),
                    stats: None,
                })
            }
            Err(e) => Err(Self::map_error(&e)),
        }
    }

    fn detect_format(&self, bytes: &[u8]) -> Option<String> {
        if let Some(format) = anydoc::Format::from_bytes(bytes) {
            return match format {
                anydoc::Format::Doc => Some("doc".to_string()),
                anydoc::Format::Docx => Some("docx".to_string()),
                anydoc::Format::Odt => Some("odt".to_string()),
                anydoc::Format::Pdf => Some("pdf".to_string()),
                anydoc::Format::Ppt => Some("ppt".to_string()),
                anydoc::Format::Pptx => Some("pptx".to_string()),
                anydoc::Format::Rtf => Some("rtf".to_string()),
                anydoc::Format::Epub => Some("epub".to_string()),
                anydoc::Format::Excel => Some("xlsx".to_string()),
                anydoc::Format::Ods => Some("ods".to_string()),
                anydoc::Format::Odp => Some("odp".to_string()),
                anydoc::Format::Csv => Some("csv".to_string()),
            };
        }

        if bytes.len() < 4 {
            return None;
        }

        if &bytes[0..4] == b"PK\x03\x04" && bytes.len() > 26 {
            if let Some(t) = bytes.get(6..26) {
                if t.starts_with(b"ppt/") {
                    return Some("pptx".to_string());
                } else if t.starts_with(b"word/") {
                    return Some("docx".to_string());
                } else if t.starts_with(b"xl/") {
                    return Some("xlsx".to_string());
                } else if t.starts_with(b"mimetype") {
                    return Some("epub".to_string());
                }
            }
        }

        if &bytes[0..4] == b"%PDF" {
            return Some("pdf".to_string());
        }

        if bytes.starts_with(b"ID3\x0A\x0D\x0A\x1A\x0A") {
            return Some("epub".to_string());
        }

        let raw = String::from_utf8_lossy(bytes);
        if raw.starts_with("<") || raw.starts_with("<!DOCTYPE") || raw.starts_with("<!doctype") {
            return Some("html".to_string());
        }
        if raw.starts_with("{") {
            return Some("json".to_string());
        }
        if raw.starts_with("<?xml") {
            return Some("xml".to_string());
        }
        if raw.starts_with("version") {
            return Some("odt".to_string());
        }

        None
    }
}
