use std::path::Path;

use async_trait::async_trait;

use crate::engine::DocumentEngine;
use crate::file_utils::{self, ASSET_DIR_NAME};
use crate::markdown_pipeline;
use crate::models::document::{Asset, Document};
use crate::models::task::{
    Cancellation, ConversionError, ConversionResult, ConversionStage, ConversionStats,
    ConversionTask, ErrorCode, ProgressCallback,
};

/// Serializes output-path allocation and writes across concurrent conversions.
/// Output naming is check-then-act, so without this two conversions of files
/// with the same stem could resolve to the same path.
static OUTPUT_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// 转换源文件的大小上限：`fs::read` 会把整个文件读进内存（解析期间还会
/// 再派生 2-3 份图像字节副本），超过上限直接拒绝而不是先吃满内存再失败。
const MAX_SOURCE_BYTES: u64 = 200 * 1024 * 1024;

/// 原子写入：先写 `<path>.tmp` 再 rename，进程在写入中途崩溃也不会留下
/// 截断的目标文件。Windows 上 rename 不能覆盖已存在的目标，先删旧目标
/// （两次操作之间的窗口极短，且远好于留下半个文件）。
fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let mut tmp = path.as_os_str().to_os_string();
    tmp.push(".tmp");
    let tmp = std::path::PathBuf::from(tmp);
    std::fs::write(&tmp, bytes).map_err(|e| e.to_string())?;
    if path.exists() {
        if let Err(e) = std::fs::remove_file(path) {
            let _ = std::fs::remove_file(&tmp);
            return Err(e.to_string());
        }
    }
    std::fs::rename(&tmp, path)
        .map_err(|e| { let _ = std::fs::remove_file(&tmp); e.to_string() })
}

/// Local document-to-Markdown engine backed by [anydoc](https://github.com/firecrawl/anydoc),
/// a pure-Rust converter with no ML models and no external services.
///
/// Embedded images are extracted to an `assets/` directory next to the output
/// file. anydoc renders embedded images as their alt text only (the Markdown
/// serializer never inlines bytes), so inline references are restored
/// best-effort by matching each image's alt text in reading order — exact for
/// unique alt texts, skipped silently when no match is found.
pub struct AnyDocEngine;

impl AnyDocEngine {
    pub fn new() -> Self {
        AnyDocEngine
    }
}

impl Default for AnyDocEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// An embedded image occurrence in reading order: its alt text plus the index
/// of the asset that carries its bytes (`anydoc::model::AssetId`).
struct InlineImageRef {
    alt: String,
    asset_id: usize,
}

/// An asset extracted to disk: our own model record plus where it was placed
/// relative to the output `.md` file.
struct WrittenAsset {
    file_name: String,
    asset: Asset,
}

fn conv_error(code: ErrorCode, message: impl Into<String>) -> ConversionError {
    ConversionError {
        code,
        message: message.into(),
        stage: ConversionStage::Parsing,
        retryable: false,
        page: None,
    }
}

/// Map anydoc's typed errors onto the app-wide error codes.
fn map_convert_error(err: anydoc::ConvertError) -> ConversionError {
    let (code, message) = match err {
        anydoc::ConvertError::Encrypted => (
            ErrorCode::Encrypted,
            "文档已加密，请提供未加密版本后重试".to_string(),
        ),
        anydoc::ConvertError::Unsupported(what) => (
            ErrorCode::Unsupported,
            format!("不支持的文件: {what}"),
        ),
        anydoc::ConvertError::Malformed { detail, .. } => (
            ErrorCode::Malformed,
            format!("文档结构损坏或不完整: {detail}"),
        ),
        anydoc::ConvertError::ResourceLimit { limit, detail } => (
            ErrorCode::Malformed,
            format!("文档超出安全限制 ({limit}): {detail}"),
        ),
        anydoc::ConvertError::MissingPart { part } => (
            ErrorCode::Malformed,
            format!("文档缺少必要部分: {part}"),
        ),
        anydoc::ConvertError::Io(e) => (
            ErrorCode::IoError,
            format!("读取文档失败: {e}"),
        ),
        other => (ErrorCode::EngineError, other.to_string()),
    };
    conv_error(code, message)
}

/// File name for an embedded asset inside the output `assets/` directory.
fn asset_file_name(index: usize, media_type: &str, origin_part: &str) -> String {
    let ext: String = match media_type {
        "image/png" => "png".to_string(),
        "image/jpeg" | "image/jpg" => "jpg".to_string(),
        "image/gif" => "gif".to_string(),
        "image/bmp" | "image/x-bmp" => "bmp".to_string(),
        "image/webp" => "webp".to_string(),
        "image/svg+xml" => "svg".to_string(),
        "image/tiff" => "tiff".to_string(),
        "image/x-emf" | "image/emf" => "emf".to_string(),
        "image/x-wmf" | "image/wmf" => "wmf".to_string(),
        _ => Path::new(origin_part)
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_ascii_lowercase())
            .filter(|e| e.len() <= 5 && e.chars().all(|c| c.is_ascii_alphanumeric()))
            .unwrap_or_else(|| "bin".to_string()),
    };
    format!("image-{index:03}.{ext}")
}

/// Collect embedded-image occurrences from the document body in reading order.
fn collect_embedded_images(
    blocks: &[anydoc::model::Block],
    out: &mut Vec<InlineImageRef>,
) {
    for block in blocks {
        match block {
            anydoc::model::Block::Heading { content, .. } => {
                collect_inline_images(content, out);
            }
            anydoc::model::Block::Paragraph(inlines) => collect_inline_images(inlines, out),
            anydoc::model::Block::List(list) => {
                for item in &list.items {
                    collect_embedded_images(&item.blocks, out);
                }
            }
            anydoc::model::Block::Table(table) => {
                for row in &table.grid {
                    for slot in row {
                        if let anydoc::model::CellSlot::Origin(cell) = slot {
                            collect_embedded_images(&cell.blocks, out);
                        }
                    }
                }
            }
            anydoc::model::Block::BlockQuote(inner) => collect_embedded_images(inner, out),
            _ => {}
        }
    }
}

fn collect_inline_images(inlines: &[anydoc::model::Inline], out: &mut Vec<InlineImageRef>) {
    for inline in inlines {
        match inline {
            anydoc::model::Inline::Image { alt, source } => {
                if let anydoc::model::ImageSource::Asset(id) = source {
                    out.push(InlineImageRef {
                        alt: alt.clone(),
                        asset_id: id.0,
                    });
                }
            }
            anydoc::model::Inline::Link { content, .. } => collect_inline_images(content, out),
            _ => {}
        }
    }
}

/// Best-effort restoration of inline image references: anydoc renders an
/// embedded image as its alt text on a line of its own, so walk the rendered
/// Markdown line by line and replace the next line whose trimmed content
/// EXACTLY equals the image's alt text (reading order). A bare substring
/// `find` would corrupt body text that merely contains the same word.
fn restore_image_references(markdown: String, refs: &[(String, String)]) -> String {
    if refs.is_empty() {
        return markdown;
    }
    let mut lines: Vec<String> = markdown.lines().map(|l| l.to_string()).collect();
    let mut cursor = 0usize;
    for (alt, rel_path) in refs {
        let needle = alt.trim();
        if needle.is_empty() {
            continue;
        }
        let matched = (cursor..lines.len()).find(|&i| lines[i].trim() == needle);
        if let Some(i) = matched {
            let label = needle.replace('[', "\\[").replace(']', "\\]");
            lines[i] = format!("![{label}]({rel_path})");
            cursor = i + 1;
        }
    }
    let mut out = lines.join("\n");
    if markdown.ends_with('\n') && !out.ends_with('\n') {
        out.push('\n');
    }
    out
}

#[async_trait]
impl DocumentEngine for AnyDocEngine {
    fn name(&self) -> &str {
        "anydoc"
    }

    fn is_available(&self) -> bool {
        true
    }

    async fn convert(
        &self,
        task: &ConversionTask,
        on_progress: Option<ProgressCallback>,
        cancelled: Option<&Cancellation>,
    ) -> Result<ConversionResult, ConversionError> {
        let report = |progress: f32, detail: &str| {
            if let Some(cb) = on_progress.as_ref() {
                cb(progress, Some(detail.to_string()));
            }
        };
        // Cooperative cancellation checkpoint; both single-file and batch
        // callers treat this error code as "cancelled", not "failed".
        let check_cancelled = |stage: ConversionStage| -> Result<(), ConversionError> {
            if cancelled.map(|c| c.cancelled()).unwrap_or(false) {
                return Err(conv_error(ErrorCode::Cancelled, "任务已取消"));
            }
            let _ = stage;
            Ok(())
        };

        check_cancelled(ConversionStage::Queued)?;
        report(0.05, "正在读取文件…");

        let source = Path::new(&task.source_path);
        let source_meta = std::fs::metadata(source)
            .map_err(|e| conv_error(ErrorCode::IoError, format!("读取源文件失败: {e}")))?;
        if source_meta.len() > MAX_SOURCE_BYTES {
            return Err(conv_error(
                ErrorCode::Unsupported,
                format!(
                    "文件过大（{} MB），超出转换上限（{} MB）",
                    source_meta.len() / (1024 * 1024),
                    MAX_SOURCE_BYTES / (1024 * 1024)
                ),
            ));
        }
        let bytes = std::fs::read(source)
            .map_err(|e| conv_error(ErrorCode::IoError, format!("读取源文件失败: {e}")))?;
        let size_bytes = bytes.len() as u64;

        // Content-based detection first; the extension is the fallback for
        // signature-less formats (CSV).
        let format = anydoc::Format::from_bytes(&bytes)
            .or_else(|| anydoc::Format::from_path(source))
            .ok_or_else(|| {
                conv_error(
                    ErrorCode::Unsupported,
                    "无法识别的文件格式：内容与扩展名均不匹配任何受支持的类型",
                )
            })?;

        check_cancelled(ConversionStage::Parsing)?;
        report(0.25, "正在解析文档…");

        // PDFs convert straight to Markdown through pdf-inspector and have no
        // document-model form (so also no assets); everything else is parsed
        // twice at ~ms cost: once for rendered Markdown, once for the model
        // that carries the embedded image bytes.
        let (raw_markdown, doc) = if format == anydoc::Format::Pdf {
            let md = anydoc::to_markdown_bytes(&bytes, format).map_err(map_convert_error)?;
            (md, None)
        } else {
            let md = anydoc::to_markdown_bytes(&bytes, format).map_err(map_convert_error)?;
            let doc = anydoc::to_document(&bytes, format).ok();
            (md, doc)
        };

        check_cancelled(ConversionStage::Parsing)?;
        report(0.55, "正在提取内嵌图片…");

        let mut written: Vec<WrittenAsset> = Vec::new();
        let mut inline_refs: Vec<(String, String)> = Vec::new();
        if let Some(doc) = &doc {
            let names: Vec<String> = doc
                .assets
                .iter()
                .enumerate()
                .map(|(i, a)| asset_file_name(i + 1, &a.media_type, &a.origin_part))
                .collect();

            let mut occurrences: Vec<InlineImageRef> = Vec::new();
            collect_embedded_images(&doc.blocks, &mut occurrences);
            for img in occurrences {
                if let Some(name) = names.get(img.asset_id) {
                    inline_refs.push((img.alt, format!("{ASSET_DIR_NAME}/{name}")));
                }
            }

            for (i, a) in doc.assets.iter().enumerate() {
                written.push(WrittenAsset {
                    file_name: names[i].clone(),
                    asset: Asset {
                        name: format!("image-{:03}", i + 1),
                        extension: names[i]
                            .rsplit('.')
                            .next()
                            .unwrap_or("bin")
                            .to_string(),
                        bytes: a.bytes.clone(),
                        media_type: a.media_type.clone(),
                    },
                });
            }
        }

        let markdown = restore_image_references(raw_markdown, &inline_refs);
        let markdown = markdown_pipeline::process(&markdown);

        let stats = ConversionStats {
            // Actual assets written to disk (not the number of inline refs,
            // which could double-count or miss unreferenced assets).
            image_count: written.len(),
            table_count: markdown_pipeline::count_table_separators(&markdown),
            word_count: markdown_pipeline::count_words(&markdown),
        };

        check_cancelled(ConversionStage::Saving)?;
        report(0.85, "正在保存文件…");

        // Layout: with images → `{out}/{stem}/{stem}.md` + `{out}/{stem}/assets/`,
        // without → flat `{out}/{stem}.md`.
        let out_dir = Path::new(&task.output_path)
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .to_string_lossy()
            .to_string();
        let has_assets = !written.is_empty();

        // `get_output_path_with_assets` is check-then-act (TOCTOU): two parallel
        // conversions of the same stem could otherwise pick the same target and
        // clobber each other. Serialize only the fast path-allocation + write
        // phase (parsing above is not under this lock).
        let md_path = {
            let _guard = OUTPUT_LOCK.lock().unwrap_or_else(|e| e.into_inner());
            let (md_path, asset_dir) =
                file_utils::get_output_path_with_assets(&task.source_path, &out_dir, has_assets);

            if let (Some(dir), true) = (&asset_dir, has_assets) {
                std::fs::create_dir_all(dir)
                    .map_err(|e| conv_error(ErrorCode::IoError, format!("创建图片目录失败: {e}")))?;
                for w in &written {
                    write_atomic(&dir.join(&w.file_name), &w.asset.bytes).map_err(|e| {
                        conv_error(
                            ErrorCode::IoError,
                            format!("写入图片 {} 失败: {e}", w.file_name),
                        )
                    })?;
                }
            }

            if let Some(parent) = md_path.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| conv_error(ErrorCode::IoError, format!("创建输出目录失败: {e}")))?;
            }
            write_atomic(&md_path, markdown.as_bytes())
                .map_err(|e| conv_error(ErrorCode::IoError, format!("写入 Markdown 失败: {e}")))?;
            md_path
        };

        report(1.0, "转换完成");

        let file_name = source
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
        let mut document = Document::new(
            &file_name,
            &format!("{format:?}"),
            size_bytes,
        );
        // `document` 只会被序列化发给前端，而 `Asset::bytes` 是
        // `#[serde(skip)]`：这里复制元数据即可，省去一份完整的图片字节拷贝。
        document.assets = written
            .iter()
            .map(|w| Asset {
                name: w.asset.name.clone(),
                extension: w.asset.extension.clone(),
                bytes: Vec::new(),
                media_type: w.asset.media_type.clone(),
            })
            .collect();

        Ok(ConversionResult {
            task_id: task.id.clone(),
            markdown,
            document,
            assets: written.into_iter().map(|w| w.asset).collect(),
            errors: Vec::new(),
            output_path: md_path.to_string_lossy().to_string(),
            stats: Some(stats),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn converts_csv_to_markdown_table() {
        let dir = std::env::temp_dir().join(format!("omnimd_anydoc_test_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let src = dir.join("sample.csv");
        std::fs::write(&src, "name,age\nalice,30\n").unwrap();

        let output_path = file_utils::get_output_path(src.to_str().unwrap(), dir.to_str().unwrap());
        let task = ConversionTask::new(src.to_str().unwrap(), &output_path);

        let engine = AnyDocEngine::new();
        let result = engine.convert(&task, None, None).await.unwrap();

        assert!(result.markdown.contains("alice"), "markdown: {}", result.markdown);
        assert!(Path::new(&result.output_path).exists(), "output not written");
        assert_eq!(result.stats.as_ref().unwrap().table_count, 1);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn unsupported_extension_fails_cleanly() {
        let dir = std::env::temp_dir().join(format!("omnimd_anydoc_test2_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let src = dir.join("notes.txt");
        std::fs::write(&src, "hello").unwrap();
        let output_path = file_utils::get_output_path(src.to_str().unwrap(), dir.to_str().unwrap());
        let task = ConversionTask::new(src.to_str().unwrap(), &output_path);

        let engine = AnyDocEngine::new();
        let err = engine.convert(&task, None, None).await.unwrap_err();
        assert_eq!(err.code, ErrorCode::Unsupported);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn restore_replaces_alt_text_in_order() {
        let md = "before\nlogo\nafter\nlogo\n".to_string();
        let refs = vec![
            ("logo".to_string(), "assets/image-001.png".to_string()),
            ("logo".to_string(), "assets/image-002.png".to_string()),
        ];
        let restored = restore_image_references(md, &refs);
        assert!(restored.contains("![logo](assets/image-001.png)"));
        assert!(restored.contains("![logo](assets/image-002.png)"));
    }

    #[test]
    fn restore_does_not_touch_body_text_mentioning_alt() {
        // 正文句子里出现的同名词不是图片占位，必须保持原样。
        let md = "the logo is iconic\nlogo\ntail".to_string();
        let refs = vec![("logo".to_string(), "assets/image-001.png".to_string())];
        let restored = restore_image_references(md, &refs);
        assert_eq!(
            restored,
            "the logo is iconic\n![logo](assets/image-001.png)\ntail"
        );
    }
}
