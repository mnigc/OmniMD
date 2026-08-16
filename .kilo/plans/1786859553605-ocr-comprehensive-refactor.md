# OCR 完善实施计划

## 目标

完善 OmniMD 的 OCR 功能，满足 `docs/OmniMD_OCR_开发需求文档_v2.md` (§1–§42)。当前引擎已更换为 PP-OCRv6 + ONNX Runtime (oar-ocr 0.9.1)，本计划聚焦**缺失的后处理链路**与**架构补齐**。

## 已确认决策

| 决策 | 选项 |
|------|------|
| 后处理范围 | **全包含**：阅读顺序、标题识别、段落合并、表格识别、页眉页脚清理 |
| 混合 PDF 策略 | **严格逐页**：`has_valid_text_layer(page)` 逐页判定 |
| 中间层设计 | `OcrResult → Document Block` 映射，复用现有 `Document` 结构 |
| PDF 渲染 | **混合模式**：优先 lopdf 提取嵌入图片，无嵌入时用 pdfium 渲染 |
| 图片预处理 | `oar-ocr` + `image` crate（orientation + 轻量去噪/对比度） |
| 阅读顺序 | **启发式单/双栏**：基于 bbox 的简单算法 |
| 模型打包 | **随安装包**：`ocr_resources/ppocrv6/` 随仓库 |

## 当前状态 (Codebase-as-is)

```
src/ocr/
├── mod.rs          → get_engine() → PpOcrEngine 单例
├── engine.rs       → OcrEngine impl (oar-ocr), 含 PdfPageImage/BMP 工具 (残留)
└── resources.rs    → 模型路径查找 (4 文件校验)

src/models/ocr.rs   → OcrMode, OcrConfig, OcrBlock, OcrResult, Cancellation, OcrEngine trait
src/pipeline.rs     → convert_file → anydoc 或 try_ocr_to_result (文本拼接)
src/markdown_pipeline.rs → Normalize/Cleanup/AI Ready (通用, 不需改)
```

### 已完成 (Tesseract→PP-OCRv6 迁移)
- Tesseract 代码/资源已清理 (grep 0 命中)
- oar-ocr 0.9.1 + PP-OCRv6 small 模型 (det/rec/dict/ori 4 文件)
- Lazy Load 单例模式
- 取消机制 (Cancellation)
- OcrMode off/auto/always 前端适配

### 已完成 (基础结构)
- `docs/要求文档_v2.md` §2-§24 UI 文案已添加 (zh-CN.ts/en.ts)
- `frontend/src/store/useSettingsStore.ts` 已使用 `ocrMode` (auto/off/always)
- `frontend/src/types/index.ts` 已有 `ocrMode`, `detail?`, OCR stats 可选字段

### 缺失 (本计划范围)
1. **OcrResult → Document Block** 映射 (结构化中间层) — OCR 结果直接拼文本
2. **逐页文本层检测** (`has_valid_text_layer`) — 不能处理混合 PDF
3. **阅读顺序重建** — 依赖 region 返回顺序
4. **后处理**：标题识别、段落合并、表格识别、页眑页脚/页码清理
5. **OCR 统计填充** — `write_result` 中 OCR stats 全为 None
6. **图片预处理** — 仅 orientation，无去噪/对比度
7. **Per-页面 OCR** — 错误处理不支持"重试单页"
8. **Progress 详情** — frontend `task-progress` 事件缺乏 page/total 信息

---

## 实施任务

### T1: 结构化中间层 + OcrResult → Document 映射

**目标**: OCR 结果先转为 `Document` 结构化 blocks，再进入 Markdown Pipeline。

**文件修改**:
- `src/pipeline.rs`:
  - `try_ocr_to_result()` → 重命名为 `ocr_to_conversion_result()`，返回 `OcrResult`
  - 新增 `fn ocr_result_to_document(ocr_result: &OcrResult, source_path: &str, format: &str) -> Document`
  - `ocr_result_to_document` 调用 `ocr_postprocess::process_ocr_result()` 获得 blocks，再 `Block` 转 `Document`
- `src/ocr/mod.rs`: 导出 `ocr_postprocess` 子模块
- `src/ocr/postprocess.rs` (新增):
  - `pub fn process_ocr_result(result: &OcrResult) -> Vec<Block>` — 入口
  - 调用 `reorder_blocks()`、`classify_block()`、`merge_paragraphs()`
  - `Block` 映射：text→Paragraph, table→Table, header/footer→跳过

**Block 映射表**:
| OcrBlock.type | Document.Block | 说明 |
|---|---|---|
| text | Block::Paragraph | 合并相近 bbox 的段落 |
| title | Block::Heading(1) | 字体大小/bbox 启发式 |
| table | Block::Table | 来处 oar-ocr text_regions 类型 |
| header | (过滤) | 重复出现的顶部区域 |
| footer | (过滤) | 重复出现的底部区域 |
| page_num | (过滤) | 纯数字的小区域 |

---

### T2: Document Detector (逐页文本层检测)

**目标**: 混合 PDF 支持，逐页分派。

**实现方式**: 优先用 `pdfium-render` 渲染+anydoc文本提取判断；fallback `lopdf` 内容流；最终 `pdf-inspector` 逐页。

**文件修改**:
- `src/ocr/pdf_text_detection.rs` (新增):
  - `pub fn has_valid_text_layer(doc: &PdfDocument, page_idx: usize) -> bool`
  - 用 `pdfium-render` 获取页面文本长度 > 阈值(50 字符)
  - 若 API 不可用，用 `lopdf::Document::get_page_content` + 文本运算符收集
- `src/ocr/mod.rs`: 导出 `pdf_text_detection`
- `src/pipeline.rs`:
  - PDF 路径改为：逐页检测 → 有文本 anydoc 提取 → 无文本 PDFium 渲染 → OCR
  - 新增 `fn convert_hybrid_pdf(task, bytes, progress, cancelled)`

**逐页分派逻辑**:
```
for page in 0..total:
  if has_valid_text_layer(page):
    text = native_extract_page(page)  # anydoc/pdfium per-page
  else:
    image = render_page_to_image(page)  # PDFium 200-300 DPI
    ocr_result = engine.recognize_image(image)
    text = postprocess(ocr_result)
  all_text.push(text)
```

---

### T3: 阅读顺序重建

**目标**: §15 支持单栏、双栏、标题优先、表格整体。

**实现**: 启发式算法，基于 bbox 位置。

**文件修改**:
- `src/ocr/postprocess.rs`:
  - `pub fn reorder_blocks(blocks: Vec<OcrBlock>) -> Vec<OcrBlock>`
  - `fn detect_layout(blocks: &[OcrBlock]) -> LayoutType` — 单栏/双栏检测
  - `fn sort_single_column(blocks) -> Vec` — 按 y_min 升序
  - `fn sort_double_column(blocks) -> Vec` — 按 x 中心分左右列，再各列按 y 排序
  - 表格区域整体保持 (table block type 不拆分)

**算法**:
1. 计算所有 blocks 的 x 范围中心分布
2. 若存在明显双峰分布 → 双栏，split 点 = 两峰谷底
3. 单栏：按 (y_min, x_min) 排序
4. 标题优先：标题块提前到同 y 层的文本前

---

### T4: 分类 + 标题识别 + 表格识别

**目标**: §16-§19

**文件修改**:
- `src/ocr/postprocess.rs`:
  - `fn classify_block(region: &TextRegion) -> BlockType` — text/title/table/header/footer/page_num
  - `fn detect_title(block: &OcrBlock, neighbors: &[OcrBlock]) -> bool` — 基于字体大小(y轴 bbox 高度)、水平居中、文本长度<阈值
  - 表格识别：依赖 `oar-ocr` 返回的 `TextRegion` 是否有 table 类型；若无，P0 按行/列合并启发式

**标题识别准则** (保守, §16 原则):
- bbox height < 平均行高 × 0.7
- 文本长度 < 200 字符
- 位于页顶部 (y_min < 页高 × 0.15)
- 不是纯数字

**表格识别**:
- 若 `TextRegion` 有 block_type=="table" → 直接映射为 `Block::Table`
- 否则：P0 保守，将表格区域的所有 text region 合并为 Paragraph

---

### T5: 段落合并 + 页眉页脚清理

**目标**: §17-§18

**文件修改**:
- `src/ocr/postprocess.rs`:
  - `fn merge_paragraphs(blocks: Vec<OcrBlock>) -> Vec<OcrBlock>` — 合并相邻 y 相连、x 重叠的行
  - `fn detect_headers_footers(blocks: &[OcrBlock]) -> (Vec<usize>, Vec<usize>)` — 重复出现在多页顶部/底部
  - `fn is_page_number(block: &OcrBlock) -> bool` — 纯数字/页码模式

**段落合并逻辑**:
- 同行 (y 重叠) 相邻行 → 合并为段落
- 跨行合并: y_min(当前) ≈ y_max(上一行) + small_gap AND x overlap > 50%

**页眉页脚检测**:
- 第一页/最后一页 top 15% / bottom 15% 区域
- 出现在 ≥2 页且文本相近 → 识别为 header/footer
- 单页纯数字且 bbox 很小 → page_num

---

### T6: 图片预处理管线

**目标**: §13 自动旋转、去噪、对比度增强、分辨率调整

**文件修改**:
- `src/ocr/preprocess.rs` (新增):
  - `pub fn preprocess_image(bytes: &[u8]) -> Vec<u8>` — 返回 PNG bytes
  - `fn correct_orientation(img: &DynamicImage) -> DynamicImage` — 用 image crate EXIF/rotation
  - `fn enhance_contrast(img: &DynamicImage) -> DynamicImage` — CLAHE 或简单拉直
  - `fn reduce_noise(img: &DynamicImage) -> DynamicImage` — 中值滤波
  - PDF 页面渲染后 → preprocess → OCR

**注意**: oar-ocr 0.9.1 已内置 doc_ori 模型 (orientation classification)。这里补充 image crate 级别的像素预处理。

---

### T7: PDF 渲染优化 + Per-页面 OCR

**目标**: §10-§11，混合渲染模式，页面级进度与错误处理

**文件修改**:
- `src/ocr/pdf_renderer.rs` (新增): —— 提取自当前 engine.rs 的 PDF 相关代码
  - `pub fn extract_embedded_images(doc: &lopdf::Document) -> Vec<PdfPageImage>` (当前已有)
  - `pub async fn render_pdf_pages(bytes: &[u8], dpi: u32) -> Vec<(Vec<u8>, usize)>` — PDFium 渲染
  - `pub fn render_page(doc, page_idx, dpi) -> Vec<u8>` — 单页渲染

- `src/pipeline.rs` → `convert_hybrid_pdf()`:
  - 使用 PDFium 打开 PDF
  - 逐页: `has_valid_text_layer(page)` → native extract / render → preprocess → OCR
  - 页面间释放内存 (`drop(bitmap), drop(page)`)
  - progress 回调: `cb(0.4 + (page_idx / total) as f32 * 0.5)`

---

### T8: 进度与错误处理优化

**目标**: §23-§26，显示当前页/总页数，支持单页重试，明确错误

**文件修改**:
- `src/pipeline.rs`:
  - `convert_file()`: progress 回调改为支持 `detail` 字段
  - 失败页记录到 `result.errors: Vec<ConversionError>` (page 级别)
  - `write_result()`: 填充 `ocr_page_count`, `ocr_char_count`, `avg_confidence_permille`, `low_confidence_count`
  - 错误结构增加 `page: Option<u32>` 字段

- `src/models/task.rs`:
  - `ConversionError` 增加 `page: Option<u32>` 字段

- `src/lib.rs`:
  - `TaskProgressDto` 回调增加 `detail: Option<String>` — 如 "OCR 第 3/18 页"
  - `ConversionStatsDto` 保持 (前端 types 已就绪)

**Progress 回调协议**:
```
cb(0.1)  → DetectingFormat
cb(0.4)  → Extracting (native)
cb(0.5..0.9) → OCR 第 x/y 页 (detail)
cb(1.0)  → Saving
```

---

### T9: oar-ocr API 适配确认

**前提检查** (需在实现前确认):

1. `oar_ocr::OAROCRBuilder::new(det, rec, dict)` — 参数顺序/签名
2. `TextRegion` 字段:
   - `text()` / `text_with_confidence()` — 已在 engine.rs 0.9.1 API 验证
   - `bounding_box()` / `aabb()` — 确认返回 `(x_min, y_min, x_max, y_max)`
   - block_type 字段 (table/text/etc.) — 若不存在，P0 全部归为 text
3. `OAROCR::predict(vec![image])` 返回 `Vec<OcrPageResult>`，每个含 `text_regions`
4. orientation model 接口: `with_document_image_orientation_classification(path)`

**验证命令**:
```bash
cd D:\project\OmniMD
cargo doc --open -p oar-ocr  # 查看 API 文档
# 或
rg "pub fn|pub struct|pub enum" ~/.cargo/registry/src/*/oar-ocr-0.9.1/src/
```

---

### T10: 测试

**现有测试**:
- `tests/converter_test.rs` — format detection, basic conversion
- `tests/file_utils_test.rs` — output path, file listing
- `src/markdown_pipeline.rs` — 单元测试 (normalize/cleanup/toc)

**新增测试**:
- `tests/ocr_test.rs`:
  - `test_ocr_mode_default_auto`
  - `test_text_layer_detection()` — mock PDF 字节
  - `test_ocr_result_to_document()` — mock OcrResult → Document blocks
  - `test_reorder_single_column()` / `test_reorder_double_column()`
  - `test_classify_block()`
  - `test_merge_paragraphs()`
  - `test_detect_page_numbers()`

**验证命令**:
```bash
cd D:\project\OmniMD
cargo test
cd frontend
pnpm build  # 类型检查
```

---

### T11: 代码清理 (Residue 清理)

**清理当前 engine.rs 残留**:
`src/ocr/engine.rs` 202-393 行包含 Tesseract 时代遗留的:
- `PdfPageImage` 结构体
- `extract_pdf_images()`
- `encode_as_bmp()`
- `is_pdf_bytes()`
- `detect_image_format()`

→ 移动 PDF 相关到 `pdf_renderer.rs`，image format 检测到 utils。

`src/pipeline.rs` 17:
```rust
use crate::ocr::engine::is_pdf_bytes;  // 改为 crate::ocr::pdf_renderer::is_pdf
```

---

## 文件修改清单

| 文件 | 修改内容 | 任务 |
|------|---------|------|
| `src/ocr/mod.rs` | 导出 postprocess、pdf_text_detection、pdf_renderer、preprocess | T2,3,4,7 |
| `src/ocr/postprocess.rs` | **新增** — 阅读顺序、分类、段落合并、页眉页脚 | T3,4,5 |
| `src/ocr/pdf_text_detection.rs` | **新增** — 逐页文本层检测 | T2 |
| `src/ocr/pdf_renderer.rs` | **新增** — PDF 页面渲染 (从 engine.rs 提取) | T7 |
| `src/ocr/preprocess.rs` | **新增** — 图片预处理 | T6 |
| `src/ocr/engine.rs` | 删除残留工具代码 (202-393) | T11 |
| `src/pipeline.rs` | 重构 convert_file, 新增 convert_hybrid_pdf, 填充 stats | T1,7,8 |
| `src/models/task.rs` | ConversionError 加 page 字段 | T8 |
| `src/models/ocr.rs` | OcrBlock 加 confidence polygon(可选) | — |
| `src/lib.rs` | progress 回调加 detail | T8 |
| `tests/ocr_test.rs` | 新增测试 | T10 |
| `Cargo.toml` | 确认已有 oar-ocr、pdfium-bundled、image、lopdf | — |

---

## 风险与对策

| 风险 | 对策 | 负责章节 |
|------|------|----------|
| oar-ocr 0.9.1 API 不确定 | **T9** 前查阅 crate 源码/Docs | T2-T5 |
| pdfium-render 200-300 DPI 内存大 | 单页处理+释放, 配置为 150-200 DPI P0 | T7 |
| 混合 PDF 逐页检测性能 | P0 用 anydoc 页级文本提取; P1 加缓存 | T2 |
| 阅读顺序算法误判 | 保守: 乱序 < 错误 (v2 §15 原则) | T3 |
| 标题识别误伤正文 | 保守: 误判阈值高, v2 §16 "宁可漏识" | T4 |
| 后处理复杂度扩大 | 增量实现: P0 text→Paragraph; P1 加 title/table/header | T1,4 |
| Windows DLL 缺失 | oar-ocr 0.9 用 ort 2.0; cargo 在编译期下 ONYX Runtime DLL | 已验证 (plan-T6) |

---

## 执行顺序

```
T9 → T1 → T11 → T2 → T7 → T3 → T4 → T5 → T6 → T8 → T10
```

**Phase 1 (P0 — MVP)**:
1. T9 (API 确认) → T2 (逐页检测) → T7 (渲染) → T1 (OcrResult→Document) → T11 (清理)
2. T10 (编译+基础测试)

**Phase 2 (P1 — 质量)**:
3. T3 (阅读顺序) → T4 (分类) → T5 (合并/清理) → T6 (预处理) → T8 (进度/错误) → T10

---

## 验收标准 (v2 §36)

- ✅ 普通 PDF 有文本层 → anydoc (不 OCR)
- ✅ 扫描 PDF → 自动 OCR + 逐页进度
- ✅ 混合 PDF → 逐页分派 (native text / OCR image)
- ✅ 图片 OCR → 支持 PNG/JPG
- ✅ 中文/英文 → PP-OCRv6 small 模型
- ✅ 自动方向 → doc_ori 模型
- ✅ 取消 → 页面间 checkpoint 可取消
- ✅ OCR 失败 → 明确错误 (含 page 号)
- ✅ 进入 Markdown Pipeline → Normalize/Cleanup/AI Ready
- ✅ 输出统计 → 页数/OCR页/字符/低置信度
- ✅ 阅读顺序基本正确 → 启发式单/双栏
- ✅ 段落合并 → 相邻行合并
- ✅ 标题保留 → bbox 启发式
- ✅ 表格尽可能 → Table block
- ✅ 页眉页脚清理 → 重复区域过滤
- ✅ `cargo test` + `pnpm build` 通过
- ✅ grep `tesseract|winget` = 0 命中