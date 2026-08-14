use serde::{Deserialize, Serialize};
use std::time::SystemTime;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Block {
    Heading { level: u8, text: String },
    Paragraph { content: String },
    BulletList { items: Vec<String> },
    OrderedList { items: Vec<String> },
    CodeBlock { lang: String, content: String },
    Table { headers: Vec<String>, rows: Vec<Vec<String>> },
    Image { src: String, alt: String },
    Quote { content: String },
    HorizontalRule,
    PageBreak,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentMetadata {
    pub file_name: String,
    pub format: String,
    pub size_bytes: u64,
    pub converted_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
    pub metadata: DocumentMetadata,
    pub blocks: Vec<Block>,
    pub assets: Vec<Asset>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Asset {
    pub name: String,
    pub extension: String,
    pub bytes: Vec<u8>,
    pub media_type: String,
}

impl Document {
    pub fn new(file_name: &str, format: &str, size_bytes: u64) -> Self {
        let timestamp = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
            .to_string();

        Document {
            metadata: DocumentMetadata {
                file_name: file_name.to_string(),
                format: format.to_string(),
                size_bytes,
                converted_at: timestamp,
            },
            blocks: Vec::new(),
            assets: Vec::new(),
        }
    }
}
