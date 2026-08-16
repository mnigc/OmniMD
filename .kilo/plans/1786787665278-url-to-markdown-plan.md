# URL → Markdown 实现计划

## 1. 背景与现状

当前"粘贴链接地址"功能 (`lib.rs:287-334` 的 `download_url` 命令) 只是把远程文件下载到本地临时目录，然后当作本地文件加入转换队列。这与需求文档第 8 节要求的 **URL → 网页抓取 → 正文提取 → 结构化 Markdown** 完全不符。

## 2. 目标

实现需求文档第 8 节定义的完整流程：

```
用户输入 URL → 点击"开始转换" → 获取网页 → 提取核心正文
→ 移除导航/广告/Footer/侧边栏/评论 → 转为结构化 Markdown
→ 应用 Markdown Pipeline → 保存到输出目录
```

## 3. 架构

```
Rust 后端 (新增)                   前端 (改动)
┌─────────────────────────┐    ┌──────────────────────┐
│ web_extractor.rs        │    │ tauriApi.ts          │
│  ├─ fetch_html(url)     │    │  └─ fetchUrl()       │
│  ├─ extract_content()   │    ├──────────────────────┤
│  └─ html_to_markdown()  │    │ useTaskStore.ts      │
├─────────────────────────┤    │  └─ startConversion  │
│ lib.rs                  │    │     支持 URL 任务     │
│  └─ fetch_url 命令      │    ├──────────────────────┤
├─────────────────────────┤    │ HomePage.tsx         │
│ pipeline.rs             │    │  └─ handleUrlSubmit  │
│  └─ 复用现有 Markdown   │    │    改为直接添加 URL  │
│     Pipeline + Exporter │    │    任务到队列         │
└─────────────────────────┘    └──────────────────────┘
```

## 4. 分步实现

### 4.1 Rust 后端：新增 `web_extractor` 模块

**文件**: `src/web_extractor.rs`

核心功能：
- `fetch_html(url: &str) -> Result<String>` — 使用 `reqwest` 获取网页 HTML
- `extract_content(html: &str) -> Result<ExtractedContent>` — 使用 `scraper` 解析 HTML，执行 readability 风格提取：
  - 移除 `<nav>`, `<footer>`, `<aside>`, `.sidebar`, `.advertisement`, `.cookie-banner`, `.comment` 等元素
  - 定位 `<article>`, `[role="main"]`, `<main>` 作为主内容区
  - 提取 `<title>` 作为文档标题
- `html_to_markdown(html: &str) -> String` — 将提取后的 HTML 转换为 Markdown：
  - 处理 h1-h6, p, ul/ol, a, img, pre/code, table, blockquote, hr
  - 图片引用保持原 URL (`![alt](url)`)
  - 超链接保留 (`[text](url)`)
  - 表格转为 Markdown 表格格式

**新增依赖**: `scraper` (HTML 解析 + CSS 选择器)

**输出结构**:
```rust
pub struct ExtractedContent {
    pub title: String,
    pub markdown: String,
    pub source_url: String,
}
```

### 4.2 Rust 后端：新增 `fetch_url` Tauri 命令

**修改文件**: `src/lib.rs`

- 新增 `fetch_url` 命令（在 `lib.rs` 中注册），参数：
  - `url: String` — 网页 URL
  - `output_dir: String` — 输出目录
  - `output_mode: Option<String>` — 输出模式
  - `ai_ready_opts: Option<AiReadyOptsDto>` — AI Ready 选项
- 处理流程：
  1. 从 URL 提取文件名（URL 最后一段 `/article` → `article`，或使用页面标题）
  2. 调用 `web_extractor::fetch_and_extract(url)` 获取内容
  3. 调用 `markdown_pipeline::process` 应用 Markdown Pipeline
  4. 复用 `pipeline.rs` 中的保存逻辑（`file_utils::get_output_path_with_assets` + `fs::write`）
  5. 返回 `ConversionResultDto`
- 输出路径：`{output_dir}/{title_or_filename}.md`
- 将命令注册到 `invoke_handler`

### 4.3 Rust 后端：注册新命令

**修改文件**: `src/lib.rs`

在 `invoke_handler` 中注册 `fetch_url` 命令。

### 4.4 前端：新增 API 函数

**修改文件**: `frontend/src/api/tauriApi.ts`

```typescript
export async function fetchUrl(
  url: string,
  outputDir: string,
  outputMode?: OutputMode,
  aiReadyOpts?: AiReadyOpts
): Promise<ConversionResult> {
  return invoke<ConversionResult>("fetch_url", {
    url,
    outputDir,
    outputMode: outputMode ?? null,
    aiReadyOpts: aiReadyOpts ?? null,
  });
}
```

### 4.5 前端：添加任务类型支持

**修改文件**: `frontend/src/types/index.ts`

```typescript
export type TaskType = "file" | "url";

// 在 ConversionTask 中添加：
export interface ConversionTask {
  // ... 现有字段
  taskType: TaskType;  // 新增，默认 "file"
}
```

### 4.6 前端：更新任务队列

**修改文件**: `frontend/src/store/useTaskStore.ts`

- `startConversion` 中增加 URL 任务分支：
  ```typescript
  if (task.taskType === "url") {
    result = await fetchUrl(task.sourcePath, task.outputDir, ...);
  } else {
    result = await convertFile(task.sourcePath, task.outputDir, ...);
  }
  ```
- 导入 `fetchUrl`

### 4.7 前端：更新首页 URL 处理

**修改文件**: `frontend/src/pages/HomePage.tsx`

- `handleUrlSubmit` 改为：
  1. 不再调用 `downloadUrl`
  2. 直接创建 URL 任务加入 session
  3. 任务显示 URL 作为文件名
- 添加任务时，`sourcePath` 设为 URL，`taskType` 设为 `"url"`
- 移除 `downloadUrl` 导入（如果不再需要）或保留

### 4.8 前端：更新 TaskItem 显示

**修改文件**: `frontend/src/components/TaskItem.tsx`

- URL 任务显示链接图标而非文件图标
- 显示完整 URL 或截断显示
- 移除文件扩展名标签（URL 任务无扩展名）

### 4.9 前端：更新 i18n

**修改文件**: `frontend/src/i18n/locales/zh-CN.ts`, `frontend/src/i18n/locales/en.ts`

- 将 `pasteUrl: "粘贴链接地址"` 改为 `pasteUrl: "粘贴 URL"`（更清晰）
- 新增 `urlTask: "网页"` 或类似标签

## 5. 边界情况

- **无效 URL**: 返回明确错误信息，前端显示在输入框下方
- **无网络**: `reqwest` 超时（30s），返回网络错误
- **非 HTML 资源**: 返回不支持错误（如直接输入图片 URL）
- **页面标题为空**: 使用 URL 最后一段作为文件名
- **大页面**: 设置响应体大小限制（如 10MB）
- **编码问题**: 从 HTML `<meta charset>` 或 HTTP header 检测编码

## 6. 验证步骤

1. 输入 https://example.com → 应返回 example 首页的 Markdown
2. 输入一篇博客文章 URL → 应得到干净的正文 Markdown，无导航/广告
3. 输入无效 URL → 显示红色错误提示
4. URL 任务与文件任务混合在队列中 → 均能正确转换
5. 输出模式切换（Standard / AI Ready / Obsidian）→ URL 任务正确应用
6. 隐私提示文字正确显示
7. `cargo build` 通过
8. 前端 `pnpm lint` 通过
9. 后端 `cargo test` 通过

## 7. 不变的部分

- 现有 `download_url` 命令保留（不删除，保持向后兼容，但前端不再使用）
- 现有 Markdown Pipeline（normalize → cleanup → output mode）不变
- 现有文件转换流程不变
- 现有 UI 布局不变，仅 URL 输入框功能改变
- 现有 `OutputMode` 枚举不变
- 隐私提示文案不变（已与需求文档一致）