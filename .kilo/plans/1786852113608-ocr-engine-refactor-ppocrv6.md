# OCR 引擎重构：PP-OCRv6 + ONNX Runtime（替换 Tesseract）

## 目标

满足 `docs/OmniMD_OCR_开发需求文档_v2.md`：

- 应用内只运行**一套 OCR 模型**：PP-OCRv6（Apache-2.0），本地推理用 **ONNX Runtime**。
- **彻底删除 Tesseract**（代码、资源、文案、文档、计划文件，零残留）。
- 用户无需安装 Python / Paddle / ONNX Runtime / 模型 / 环境变量（v2 §29、§38-3）。
- 保留既有外围骨架：独立模块、后台执行、取消、超时、单页失败不丢文档、统一 Markdown Pipeline。

## 已确定的决策（基于调研）

| 项 | 决策 | 依据 |
|---|---|---|
| 模型体系 | PP-OCRv6 **small**（det 9.9MB + rec 21.2MB） | v2 §2；small 支持 50 语言、识英文/简中；tiny(6MB)留作快速档 |
| 推理运行时 | `oar-ocr` crate 0.9.x（基于 `ort`/ONNX Runtime） | 纯 Rust、Apache-2.0、crates.io 活跃（2026-08 最新 0.9.1）、支持内存/路径加载模型、自带方向分类/结构分析接口 |
| 方向检测 | PP-LCNet doc_ori（6.8MB） | v2 §14 自动方向 0/90/180/270 |
| 词典 | `ppocrv6_dict.txt`（18708 字符） | PP-OCRv6 small/medium 共用 |
| 无 Python / 无第二套 OCR | 强制 | v2 §38-1/_3 |
| 模型资源位置 | `ocr_resources/ppocrv6/` | 已随仓库 `tauri.conf.json` `resources:["ocr_resources"]` 打包 |
| 工具链 | 本机 Rust 1.97.1（≥ oar-ocr MSRV 1.95） | 已核验 |

**资源已就绪**（本会话已下载，复用，勿重复下载）：
`ocr_resources/ppocrv6/`: `pp-ocrv6_small_det.onnx`、`pp-ocrv6_small_rec.onnx`、`ppocrv6_dict.txt`、`pp-lcnet_x1_0_doc_ori.onnx`

## 架构（改造后，不引入无关重构）

```text
UI(frontend) ──invoke──> lib.rs Task Manager (现有 AppState/Cancellation)
   └── Conversion Service (src/pipeline.rs)
         ├── Document Detector（新增 has_valid_text_layer 逐页判定）
         │     ├── 有文本层页 → anydoc 原生提取（现有 converter）
         │     └── 无文本层页 → OCR Service（src/ocr/）
         └── OCR Service
              ├── OCRConfig{mode:off/auto/always, language, model_path, runtime}
              ├── OCREngine 单例（Lazy Load，任务间复用）
              │     └── oar-ocr OAROCRBuilder(PP-OCRv6 det/rec/dict + doc_ori)
              └── OCRResult{page,width,height,blocks:[OcrBlock{type,text,confidence,bbox,order}]}
         └── Markdown Pipeline（现有 Normalize→Cleanup→AI Ready，不改逻辑）
```

## 实施任务（按序执行，每阶段可验证）

### T0 清理旧 Tesseract 残留（删除，不留任何引用）

1. 删除整目录 `ocr_resources/tesseract/`（含 tessdata/*.traineddata）。
2. 删除旧计划文件 `.kilo/plans/1786790000000-tesseract-ocr-minimal-plan.md`。
3. `src/ocr/engine.rs`：**整体重写**（删除 `TesseractOcrEngine`、subprocess 调用、`run_tesseract_*`、`CleanupGuard`、`PdfPageImage`、`extract_pdf_images`、`encode_as_bmp`、`is_pdf_bytes`、`detect_image_format`、所有 tesseract 相关函数）。
4. `src/ocr/resources.rs`：**整体重写**（删除 tesseract.exe/tessdata/TESSDATA_PREFIX/PATH/系统安装目录探测，改为定位 `ocr_resources/ppocrv6/` 模型与 dict）。
5. `src/ocr/mod.rs`：更新导出（移除 tesseract 相关 pub use；保留 `get_engine()` 但返回新引擎单例）。
6. `build.rs`：删除创建 `ocr_resources/tesseract/tessdata` 与 `.keep` 及 winget 警告，仅保留 `tauri_build::build()`（可保留 OCR 资源目录存在性校验/预创建 `ppocrv6`）。
7. `src/pipeline.rs`：删除 "Install Tesseract via winget install tesseract" 文案与 `winget` 提示分支；改由 `OcrFailed.not_available` 文案统一接管。
8. 全局搜索 `tesseract|Tesseract|TESSDATA|tessdata|winget install`（`src/`、`frontend/src/`、`*.md`、`Cargo.toml`、`tauri.conf.json`、`build.rs`）确保为 0 命中（除 git 历史）。同步清理 `docs/开发调试指南.md`、`README*.md` 若含 OCR/Tesseract 描述。
9. 确认 `Cargo.toml` 无 tesseract 相关依赖（现无，无需动 lopf——lopf 保留用于 PDF 嵌入图提取）。

### T1 OCR 抽象层（对齐 v2 Phase 2）+ 结构化结果

1. `src/models/ocr.rs`：
   - 新增 `OcrMode{Off,Auto,Always}`（serde camelCase；`Default=Auto`）。
   - `OcrConfig` 扩字段：`mode: OcrMode`、`language: String`(现状 `auto|zh|zh+en|en`)、`model_path: Option<String>`、`runtime: String`。
   - 新增 `OcrBlock{block_type,text,confidence,bbox:[f32;4]或 polygon Vec<(f32,f32)>,page:u32,order:u32}`（serde）。
   - 新增 `OcrResult{page,width,height,blocks:Vec<OcrBlock>}`。
   - 保留 `Cancellation`；`OcrEngine` trait 改造：`recognize_image(&[u8], &OcrConfig) -> Result<OcrResult>`，携带 `ProgressCallback`（0..1）与 `Cancellation` 回执。
2. `src/models/task.rs`：`ConversionTask.ocr_enabled: bool` → `ocr_mode: OcrMode`（serde default Auto）。`ConversionStats` 增 OCR 统计：`ocr_page_count/ocr_char_count/avg_confidence_permille(枚举值纵横)/low_confidence_count`（新增后端可空字段，DTO 兼容）。

### T2 OCR 引擎实现（oar-ocr + PP-OCRv6）

1. `src/ocr/engine.rs` 重写：
   - `PpOcrEngine` 持有 `OAROCRBuilder` 产物（det/rec/dict + `with_document_image_orientation_classification(doc_ori)`）。
   - **Lazy Load 单例**：首次 OCR 时才在后台构建；构建后缓存（`AppState` 存 `Mutex<Option<PpOcrEngine>>` 或请照现有 `Cancellation` 管理方式）；任务间复用，不重复加载。
   - `recognize_image`：字节 → oar-ocr `load_image`/内存解码 → `predict` → `text_regions` → `OcrBlock`。映射 `text_with_confidence()`(text,conf) 与 bbox（注意：**实现时打开 oar-ocr `OcrResult/TextRegion` 源码确认坐标字段名**；若坐标不可得，P0 先按返回顺序赋 `order`，bbox 置零并在结果内标记）。
   - 进度：按预测批次回调；取消：页间/批次检查 `Cancellation`。
   - 错误：缺失模型 → `ErrorCode::OcrFailed` + `retryable=false` + 明确"模型缺失"文案；推理失败 → `retryable=true`。
2. `src/ocr/resources.rs`：`find_model_dir()/find_rec_dict()` 定位 `ocr_resources/ppocrv6/`（manifest→exe 同目录两级，保留现有 bundle 查找模式）；`check_resources()` 校验 det/rec/dict/ori 四文件存在。
3. PDF 逐页图像源（v2 §10-11）：
   - P0：沿用 lopf `get_page_images` 提取嵌入图像（覆盖整页扫描图场景），映射每页 → `recognize_image`；按页处理、页间释放、支持取消。保留 `PdfPageImage` 结构的轻量版（从原 tesseract 版精简），保留 `encode_as_bmp`（移除 JPEG 分支依赖），只在 OCR 路径使用。
   - P1（若 P0 在回归测试中发现缺图/矢量页）：新增 `pdfium-render` crate 做 200-300 DPI 页面光栅化，逐页渲染+及时释放。
   - **实现时需确认**：pdf-inspector 是否支持逐页文本提取（决定 `has_valid_text_layer` 实现方式，见 T3）。

### T3 Document Detector（文本层检测）+ 混合 PDF（v2 §4.3/§9/§37-4/5）

1. 新增 `src/ocr/mod.rs` 或 `src/pipeline.rs` 内 `has_valid_text_layer(逐页) -> Vec<bool>`：
   - 优先候选：`pdf-inspector` 逐页文本提取，统计字符数/可打印比例（阈值：单页有效字符 > N、乱码率）。若 API 不支持逐页，则用 lopf 页面内容流解析文本（`lopdf::Document::get_page_content` + text 运算符收集）实现等价判定。
   - 图片文件（png/jpg/...）视为"无文本层"（走 OCR），与现有 `is_image_file` 分支合并。
2. `pipeline.rs`：`convert_file` 重写 OCR 回退策略：
   - `ocr_mode==Off` → 永不走 OCR（现状行为）。
   - `ocr_mode==Auto`（默认）：
     - 图片/无文本层页 → OCR；
     - 普通 PDF → anydoc；
     - **混合 PDF → 逐页分派，统一合并**（原生页文本 + OCR 页结构 → 单一 `OcrResult`/Document → Markdown Pipeline）。
   - `ocr_mode==Always` → 支持文档全部强制 OCR。
   - 保留现"乱码/超时/无文本回退 OCR"启发式（在 Auto 下）。
3. 取消 `try_ocr_to_result` 中 `String` 路径，改为 `OcrResult` 生命周期。

### T4 Markdown Pipeline 对接（v2 §21/§24，复用现有模块）

1. 新增 `OcrResult → Document blocks` 的转换（heading/paragraph/table 预留）：
   - P0：按 `order` 排序，`OcrBlock` → `Block::Paragraph`（text），跨页合并；**标题/表格/页眉页脚/段落合并为 P1 启发式**（v2 §15-19 标注为"尽可能"，P0 保持保守：全部段落）。
   - `--- Page N ---` 临时分隔写法必须移除（v2 AI Ready 要求），改用结构化 blocks。
2. 走现有 `markdown_pipeline::process`（Normalize/Cleanup/AI Ready 不改）。

### T5 UI 与状态（v2 §22/§23/§24/§25/§26）

1. `frontend/src/store/useSettingsStore.ts`：`ocrEnabled:boolean` → `ocrMode:"off"|"auto"|"always"`（默认 `auto`），持久化 key 兼容旧值（`undefined → auto`）。
2. `frontend/src/types/index.ts`：`TaskProgressDto` 增 `detail?: string`；`ConversionStats` 增 OCR 统计可选字段；`ConversionTask` 增 `ocrMode`。
3. `frontend/src/api/tauriApi.ts`：`convertFile` 参数 `ocrEnabled` → `ocrMode`。
4. `frontend/src/store/useTaskStore.ts`：透传 ocrMode；`task-progress`/`task-status` 处理 `detail`。
5. `frontend/src/pages/SettingsPage.tsx` + `frontend/src/i18n/locales/{zh-CN,en}.ts`：OCR 段改三态（自动/始终关闭/始终开启）切换 UI；补隐私文案"OCR 在本机运行…"；错误文案删 Tesseract 提及。
6. `frontend/src/components/TaskItem.tsx` / `ConvertPage.tsx`：显示阶段+页码详情（如 `detail`）；结果页显示页数/OCR页/字符/低置信度（若 DTO 提供）。

### T6 打包与许可证

1. `tauri.conf.json`：`resources` 保持 `["ocr_resources"]`（已含 `ppocrv6/`）；确认删除 `tesseract` 后打包体积以新目录为准。
2. ONNX Runtime 动态库随包（**关键风险**）：`ort` 的 `download-binaries` 在构建期下载 ORT DLL，Tauri 打包默认收集依赖 DLL；若产物中缺 `onnxruntime.dll`，将该 DLL 复制进 `ocr_resources/ort/` 并加入 `tauri.conf.json` resources 或外部 bin。
3. 新增 `THIRD_PARTY_NOTICES.md`（仓库根）：PP-OCRv6 模型(Apache-2.0, PaddlePaddle)、oar-ocr(Apache-2.0)、ort(MIT/Apache-2.0)、ONNX Runtime(MIT)、lopf(MIT)。

### T7 验证（对齐 v2 §35/§36/§37-8）

1. `cargo check` / `cargo test`（含 `tests/converter_test.rs`、`file_utils_test.rs` 不回归）。
2. `pnpm build`（frontend 类型检查）。
3. 真实回归：`样例数据/`（或自备）至少 20 个文件：普通 PDF（不进 OCR）、纯扫描 PDF、混合 PDF、中文/英文、表格、双栏、旋转页、模糊、PNG/JPG；记录耗时/内存/错误率。抽查输出无 `tesseract`/`winget`/`--- Page N ---` 残留。
4. 功能验收对照 v2 §36 清单逐项过。

## 风险与对策

| 风险 | 对策 |
|---|---|
| `oar-ocr` 0.x API 变动 / TextRegion 坐标字段不确定 | 锁版本 `=0.9.1`；实现时读 crate 源码确认 `TextRegion` 字段；坐标不可得则 P0 以返回顺序定 order |
| `ort` 2.0 为 rc（`oar-ocr 0.9` 依赖 `ort 2.0.0-rc`） | 跟随 oar-ocr 锁定解析版本；`copy-dylibs`/`ORT_LIB_LOCATION` 处理 Windows DLL 加载（备 `load-dynamic` 兜底） |
| ONNX Runtime DLL 未随安装包 → 目标机启动失败 | T6.2 显式核验产物中含 `onnxruntime.dll` 并纳入 resources |
| 混合 PDF 逐页判定无现成 API | 候选：pdf-inspector 逐页 或 lopf 页面内容流解析；P0 若混合判定成本高，先按"整文档有效文本比例"分流（单页混合放开到 P1 循环），但**扫描 PDF/图片必须有页面级进度**
| 模型随包体积增大（small ≈31MB + 方向 6.8MB） | v2 §28 允许；tiny 档(6MB)后续作为"快速模式"可插拔 |
| 旧 `ocrEnabled` 前端持久化值 | 迁移读取时 `ocrMode = parsed.ocrMode ?? "auto"` 兼容 |

## 明确不做（P1+，本计划范围外）

- 标题识别/表格识别/页眉页脚清理/段落合并启发式（markdown_pipeline 新增规则）。
- PP-StructureV3/OARStructureBuilder 结构分析接入。
- PDF 路由 200-300 DPI 渲染（除非 P0 回归暴露缺图，才启 P1 `pdfium-render`）。
- 云端 OCR / LLM / 账户系统。

## 验收退出标准

- `grep -ri tesseract|TESSDATA|tessdata|winget install` 在 src/ frontend/src/ 与所有文档中 **0 命中**（git 历史除外）。
- `cargo check`、`cargo test`、`pnpm build` 通过。
- 纯文本 PDF 默认不进 OCR；扫描 PDF / 图片自动 OCR 且带进度"第 x/y 页"；混合 PDF 分页合并；取消可用；失败有明确错误。
- 输出无 `--- Page N ---`、无 tesseract 文案，统一走现有 Markdown Pipeline。