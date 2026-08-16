# OmniMD 核心转换闭环补齐计划

## 目标
补齐需求文档中「重要且能快速闭环」的 5 项缺口，让单文件/文件夹转换的完整路径
（拖入 → 选模式 → 转换 → 统计/错误 → 复制/保存/重试）跑顺，不动 URL 正文抽取、
Windows 右键菜单、CLI、AI 优化/OCR。

## 已确认决策
1. **范围**：仅 5 项（见下）。URL/右键/CLI/AI 重活本轮不做。
2. **AI Ready formatter**：只做安全变换——可选生成目录、可选文档元信息头、连续重复行去重、剥除残留 HTML 标签、确保单 H1。页眉/页脚启发式剔除推迟到 AI/P2。
3. **assets 输出结构**：仅当文件含 assets 时才建 `name/` 子目录（`name/name.md` + `name/assets/`）；无图文件直接写 `output/name.md`，避免空子目录泛滥。Markdown 内引用统一用相对路径 `assets/xxx`。

## 涉及边界
- 后端 Rust：`src/pipeline.rs`、`src/markdown_pipeline.rs`、`src/file_utils.rs`、`src/models/task.rs`、`src/lib.rs`
- 前端 TS：`frontend/src/types/index.ts`、`store/useSettingsStore.ts`、`store/useTaskStore.ts`、`api/tauriApi.ts`、`pages/ConvertPage.tsx`、`pages/BatchPage.tsx`、`pages/SettingsPage.tsx`、`pages/HomePage.tsx`、`i18n/locales/zh-CN.ts`、`i18n/locales/en.ts`
- 不动：`src/converters/anydoc_converter.rs`（渲染层保持现状）、`tauri.conf.json`、capabilities。

## 数据流 / DTO 变更
扩展 `ConversionResultDto`（`src/lib.rs`）新增统计字段，由 `pipeline::convert_file` 计算并回填：
```
pub struct ConversionStats {
    pub duration_ms: u64,        // 转换耗时
    pub source_size_bytes: u64,  // 原始大小
    pub output_size_bytes: u64,  // 输出 .md 字节数
    pub image_count: usize,      // markdown 中 ![..]( 资源引用计数（=assets 数）
    pub table_count: usize,      // markdown 中表格分隔行 `|---` 计数
    pub warning_count: usize,    // result.errors 中 retryable=true 且非致命的数量
}
```
`ConversionResultDto` 增加 `stats: ConversionStats` 与 `output_path: String`（供前端打开文件夹/重试复用，避免再算）。前端 `types/index.ts` 同步新增类型。

错误结构化：`ErrorDto` 已有 code/message/retryable。前端 ConvertPage/BatchPage 渲染时按 code 映射「建议」（查表，不需后端返回建议文案），i18n key 形如 `error.suggest.<code>`。

## 任务清单（按依赖序）

### T1. assets 输出结构改为「有图才建子目录」
- `src/file_utils.rs`：新增 `get_output_path_with_assets(src, out_dir, has_assets) -> (md_path, asset_dir_opt)`。当 `has_assets` 为真，输出 `{out_dir}/{stem}/{stem}.md`，asset_dir=`{out_dir}/{stem}/assets`；否则 `{out_dir}/{stem}.md`，asset_dir=None。
- `src/pipeline.rs::convert_file`：先 `converter.convert` 得到 result，据 `result.assets.is_empty()` 选输出路径；asset_dir 用 `Option`，写 assets 时仅在有目录时写。Markdown 内引用已是 `assets/xxx`，与新结构天然兼容，无需改 converter 渲染。
- `src/pipeline.rs::collect_folder_tasks`：文件夹递归时无法预知是否有图，统一按「有图建子目录」规则——但 collect 阶段拿不到 assets。决策：folder 模式下统一走「子目录」结构（`{out}/{rel}/{stem}/{stem}.md`），与文档 §7.2 示例一致；单文件/批量命令模式按 has_assets 决定。在 task 上记 `force_subdir: bool` 或在 convert_file 内统一逻辑：实际统一为「convert_file 内部据 has_assets 决定路径」，collect_folder_tasks 只产出 source→暂定 output_dir，最终路径由 convert_file 重算。**简化方案**：让 `convert_file` 始终在内部重算最终 output_path（覆盖 task.output_path 的文件名部分，保留目录前缀），collect_folder 只负责给每文件一个 output_dir 前缀。
- 调整现有 `tests/` 与 `pipeline.rs` 内测试，确认新路径规则。

### T2. ConversionStats 计算与 DTO 扩展
- `src/models/task.rs`：新增 `ConversionStats` 结构（上文）。`ConversionResult` 增加 `stats: Option<ConversionStats>`（默认 None，向后兼容）。
- `src/pipeline.rs::convert_file`：记录起始 `Instant`；转换完成后统计 markdown 中图片/表格数；`output_size_bytes` = markdown 字节数；`source_size_bytes` = bytes.len()；warning_count = errors.iter().filter(retryable).count()。填入 result.stats。
- `src/lib.rs`：`result_to_dto` 输出 stats 与 output_path；`ConversionResultDto` 增字段。批量回调同步带 stats。

### T3. AI Ready formatter 安全变换
- `src/markdown_pipeline.rs::format_ai_ready`：实现
  - 剥残留 HTML：行内 `<tag ...>...</tag>` 与自闭合标签用简单正则剥离（保留内容文本），代码块内不动（复用 in_code_block 状态机）。
  - 连续重复行去重：相邻完全相同非空行折叠为一行（标题行除外，避免误并标题）。
  - 确保单 H1：若出现多个 `# ` 顶层标题，仅保留第一个，其余降为 `## `。
  - 可选目录：受 settings 控制（见 T4），默认关。生成时在文首插 `## 目录` + 锚点列表（基于 H1/H2/H3 文本转 slug）。本项作为开关，pipeline 需能接收 `gen_toc: bool`。
  - 可选文档元信息头：受开关控制，默认关。生成受控的注释式元信息块（`<!-- source: ... converted: ... -->`）。
- 因 `process()` 签名目前只收 `(markdown, mode, source_path)`，需扩展为接收 `AiReadyOptions { gen_toc, gen_meta }`。从 task 或 settings 传入。决策：在 `ConversionTask` 增加 `ai_ready_opts` 字段（serde default），由 lib.rs 命令从 settings 读取后注入 task。

### T4. 设置项补齐 + 持久化
- `frontend/src/store/useSettingsStore.ts`：扩展 StoredSettings 增加 `defaultOutputDir: string`、`recursive: boolean`(默认 true)、`keepStructure: boolean`(默认 true)、`aiReadyToc: boolean`(默认 false)、`aiReadyMeta: boolean`(默认 false)、`aiEnabled: boolean`(默认 false)。load/save 同步。新增对应 setter。
- `SettingsPage.tsx`：转换卡新增「默认输出目录」（Input+浏览按钮，复用 pickOutputDir）、「递归处理文件夹」「保留原目录结构」开关；新增「AI」卡：AI 功能开关、AI Ready 目录开关、AI Ready 元信息开关（API 配置/是否允许联网本轮仅占位 disable，标 comingSoon）。
- `HomePage.tsx`：outputDir 初值改读 settings.defaultOutputDir；递归/保留结构在调用 convert_folder 时传入（需 tauriApi/命令支持，见 T5）。
- i18n：补 `settings.defaultOutputDir`、`settings.aiCard`、`settings.genToc`、`settings.genMeta`、`settings.allowOnline`（占位）等 key，zh-CN 与 en 同步。

### T5. 命令层透传新设置
- `src/lib.rs`：`convert_folder` 增加 `recursive: Option<bool>`、`keep_structure: Option<bool>`、`ai_ready_opts: Option<AiReadyOptsDto>` 参数；`convert_file`/`convert_batch` 增加 `ai_ready_opts`。
- `src/pipeline.rs::collect_folder_tasks`：接收 recursive/keep_structure；recursive=false 时只扫一层；keep_structure=false 时输出扁平到 output_dir（仍按 T1 子目录规则）。
- `tauriApi.ts`：对应函数签名补参；`types/index.ts` 增 `AiReadyOpts` 类型。
- HomePage/BatchPage 调用处补传 settings 值。

### T6. 结果页统计面板 + 结构化错误卡片
- `ConvertPage.tsx`：在底部操作栏上方加统计条：耗时/原始大小/输出大小/图片/表格/警告（用 `currentResult.stats`）。大小做人类可读格式化（KB/MB）。i18n key `convert.stats.*` 已存在，确认补 `durationMs/size` 表达。
- 错误卡片：当 `currentResult.errors` 非空或 task.error 存在时，渲染结构化卡片：文件名 + 原因(message) + 建议(按 code 查表)。新增 i18n key `error.suggest.malformed`、`error.suggest.encrypted`、`error.suggest.unsupported`、`error.suggest.ioError`、`error.suggest.ocrFailed`。
- `BatchPage.tsx`：失败项 TaskItem 已显示 error 文案；新增「重新转换失败项」按钮（见 T7）。

### T7. 批量失败重试 + 复制纯文本
- `useTaskStore.ts`：新增 `retryFailed()` action——收集 status=Failed 的 task.sourcePath，调用 convert_batch 重跑，结果回填（复用 finalizeTask 逻辑）。需保留原 outputDir/concurrency/outputMode（从 settings/当前 task 推断）。
- `BatchPage.tsx`：新增「重试失败项」按钮（failedTasks.length>0 时可点），调 `retryFailed()`。已有 Pause/Resume 保持 disabled（本轮不做真暂停）。
- `ConvertPage.tsx`：操作栏新增「复制纯文本」按钮——strip markdown 语法（轻量：去 `#`/`*`/`>`/列表标记/表格竖线/链接留文本）后写剪贴板。i18n `convert.copyPlainText` 已存在。

## 失败模式 / 风险
- **assets 子目录化破坏现有输出路径**：已有用户依赖「flat md + 共享 assets」会路径变化。缓解：仅在含图时改结构，纯文本行为不变；在 README/版本号 bump 说明。
- **collect_folder 无法预知 has_assets**：用「convert_file 内部重算最终路径」统一，避免 collect 阶段猜。folder 模式统一走子目录结构（与文档示例一致），单文件批量按 has_assets。
- **AI Ready 剥 HTML 误伤代码块**：复用 in_code_block 状态机，代码块内不做任何替换；加单测覆盖。
- **连续重复行去重误并标题**：标题行（`#` 开头）不参与去重；加单测。
- **stats 统计表格数误判**：仅计 `is_table_separator` 行（复用 markdown_pipeline 的判定），避免把正文里的 `|` 当表格。
- **settings 结构变更导致旧 localStorage 解析失败**：loadSettings 已用 `??` 兜底，新增字段同样兜底，无迁移风险。

## 验证计划
1. `cargo test`：新增/更新 `markdown_pipeline` 测试（剥 HTML、重复行去重、单 H1、目录生成）；`pipeline` 测试（assets 子目录路径、stats 字段非空）。
2. `pnpm build` + `pnpm lint`/`tsc --noEmit`（确认前端类型通过）。
3. 手动回归（真文件）：
   - 含图 DOCX → 确认 `name/name.md` + `name/assets/image-001.png`，预览图正常。
   - 纯 txt → 确认 `output/name.md`，无空子目录。
   - 文件夹递归 → 确认结构保留 + 每含图文件独立 assets。
   - 失败文件（损坏 PDF）→ 确认结构化错误卡片 + 批量「重试失败项」可重跑。
   - AI Ready 模式 → 确认目录/元信息开关生效、HTML 残留被剥、单 H1。
   - 设置页 → 默认输出目录、递归/保留结构、AI 卡开关持久化生效。

## 不在本轮范围（明确推迟）
- URL 正文抽取（§8，需 readability 库）
- Windows 右键菜单（§10，需 NSIS 注册表 + 启动参数解析）
- CLI（§13）
- AI 优化按钮实质功能 / OCR（§14，P2）
- 暂停/继续真后端取消（按钮保持 disabled）
- 页眉页脚启发式剔除（推迟 AI/P2）
