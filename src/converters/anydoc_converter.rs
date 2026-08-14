use crate::models::{
    document::{Asset, Block, Document},
    task::{ConversionError, ConversionResult, ConversionStage, ErrorCode},
    Converter,
};

pub struct AnyDocConverter;

const SUPPORTED_FORMATS: &[&str] = &[
    "pdf", "docx", "docm", "doc", "pptx", "pptm", "ppsx", "ppsm", "ppt", "pps", "pot",
    "xlsx", "xlsm", "xlsb", "xls", "epub", "csv", "txt", "html", "htm", "odt", "ods", "odp",
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
        }
    }

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

    fn map_anydoc_assets(anydoc_assets: Vec<anydoc::model::Asset>) -> Vec<Asset> {
        anydoc_assets
            .into_iter()
            .enumerate()
            .map(|(i, asset)| {
                let name = if !asset.origin_part.is_empty() {
                    std::path::Path::new(&asset.origin_part)
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| format!("asset-{:03}", i))
                } else {
                    format!("asset-{:03}", i)
                };

                let ext = name
                    .rfind('.')
                    .map_or("".to_string(), |idx| name[idx + 1..].to_string());

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
                    });
                }
                Err(e) => return Err(Self::map_error(&e)),
            }
        }

        match anydoc::to_document(bytes, anydoc_format) {
            Ok(anydoc_doc) => {
                let blocks = Self::flatten_blocks(anydoc_doc.blocks);
                let assets = Self::map_anydoc_assets(anydoc_doc.assets);

                let markdown = match anydoc::to_markdown_bytes(bytes, anydoc_format) {
                    Ok(m) => m,
                    Err(_) => String::new(),
                };

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
