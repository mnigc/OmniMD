# 计划：PDF 文本提取失败修复（TextBased PDF 返回 "OCR 所需" 错误）

## 问题

`有赞文化手册_2022.pdf` 转换时报错：
```
[Unsupported]: unsupported input: PDF has no extractable text (TextBased, 42 pages): OCR is required
```

用户确认 PDF 内的文字可以复制，说明 PDF 确实有文本层。

## 根因分析

调用链：
```
anydoc::to_markdown_bytes(bytes, Format::Pdf)
  → pdf_inspector::process_pdf_mem(bytes)
    → extract_positioned_text_with_folio_context(...)    # 带位置信息的文本提取
      → 对 TextBased PDF 逐页检查 is_cid_garbage()
        → 判定为垃圾文本，过滤掉该页所有 text items
    → 所有页面文本都被过滤后 items 为空
    → markdown = None
  → anydoc 返回 ConvertError::Unsupported("PDF has no extractable text...")
```

**`pdf_inspector` 提供了两个提取 API**：
- `process_pdf_mem()` — 带位置提取 + 垃圾检测（当前路径，42 页都被过滤了）
- `extract_text_mem()` — 直接调用 `lopdf::Document::extract_text()`，不做垃圾过滤

`extract_text_mem` 绕过了 `is_cid_garbage` 过滤，应该能提取出可读文本。

## 修复方案

在 `anydoc_converter.rs` 中，当 `anydoc::to_markdown_bytes` 对 PDF 返回 `Unsupported` 错误时，使用 `pdf_inspector::extract_text_mem` 作为回退路径提取文本。

## 涉及文件

| 文件 | 改动 | 说明 |
|------|------|------|
| `Cargo.toml` | 添加 `pdf-inspector = "1.14.2"` 依赖 | 直接依赖 pdf_inspector 以调用 `extract_text_mem` |
| `src/converters/anydoc_converter.rs` | 修改 PDF 代码路径（第 270-295 行） | 回退到 `extract_text_mem` |

## 实施步骤

### 步骤 1：添加 `pdf-inspector` 依赖

在 `Cargo.toml` 的 `[dependencies]` 段中添加：
```toml
pdf-inspector = "1.14.2"
```

注意：`pdf-inspector` 已经是 anydoc 的传递依赖，显式添加为直接依赖不引入额外编译成本。

### 步骤 2：在 `anydoc_converter.rs` 添加 PDF 回退逻辑

修改第 270-295 行的 PDF 处理代码：

**当前代码**：
```rust
if anydoc_format == Some(anydoc::Format::Pdf) {
    match anydoc::to_markdown_bytes(bytes, anydoc::Format::Pdf) {
        Ok(markdown) => {
            // ... build ConversionResult from markdown
        }
        Err(e) => return Err(Self::map_error(&e)),
    }
}
```

**修改为**：
```rust
if anydoc_format == Some(anydoc::Format::Pdf) {
    match anydoc::to_markdown_bytes(bytes, anydoc::Format::Pdf) {
        Ok(markdown) => {
            // ... build ConversionResult from markdown (不变)
        }
        Err(e) => {
            // 检查是否是 "no extractable text" 错误（TextBased PDF 被垃圾检测过滤）
            let err_msg = e.to_string();
            if err_msg.contains("no extractable text") {
                // 回退：使用 pdf_inspector 的基础文本提取（不经过垃圾过滤）
                match pdf_inspector::extract_text_mem(bytes) {
                    Ok(text) => {
                        let markdown = if text.trim().is_empty() {
                            return Err(Self::map_error(&e));
                        } else {
                            let mut md = text.clone();
                            if !md.ends_with('\n') {
                                md.push('\n');
                            }
                            md
                        };
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
                    Err(fallback_err) => {
                        tracing::warn!("PDF fallback extraction also failed: {}", fallback_err);
                        return Err(Self::map_error(&e));
                    }
                }
            }
            Err(Self::map_error(&e))
        }
    }
}
```

### 步骤 3：验证

1. `cargo check` — 确认 Rust 编译通过
2. 使用该 PDF 文件测试转换 — 确认能提取出可读文本
3. 用其他格式文件（docx、pptx、xlsx）测试 — 确认非 PDF 路径不受影响
4. 用一个已知的扫描版 PDF（无文本层）测试 — 确认仍返回 OCR 错误

## 设计决策

1. **为什么用 `pdf_inspector` 而不是 `lopdf` 直接？** `pdf_inspector::extract_text_mem` 已经封装了 PDF 加载和页码枚举逻辑，且它是 anydoc 的同源库，行为一致。直接使用 `lopdf` 需要自己处理加载和分页，多了几行代码但收益不大。
2. **只在 "no extractable text" 错误时回退** — 其他 Unsupported 错误（加密、损坏等）不应尝试回退。
3. **回退失败时返回原始错误** — 保持错误信息一致性，用户看到的还是 anydoc 的原始错误消息。
4. **回退提取的文本作为单个 Paragraph Block** — 与当前 PDF 成功路径一致（当前 PDF 路径也是把整个 markdown 当作一个 Paragraph）。

## 风险

- `extract_text_mem` 可能返回乱码（如果 ToUnicode CMap 确实损坏）— 但用户已确认文字可复制，说明 CMap 是有效的
- 回退提取的文本缺少 Markdown 格式（无标题层级、加粗等）— 这是已知权衡，比完全失败好
- 如果 PDF 部分页面可提取、部分不可，回退会提取所有可提取页面的文本，可能包含乱码页面 — 可接受的降级

## 不包含

- OCR 引擎实现（`src/models/ocr.rs` 的 trait 仍未实现）
- 设置页 OCR 开关的逻辑（仍为占位状态）