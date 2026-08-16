# OmniMD 产品与技术需求文档
## 产品方向：本地 Markdown 工作台 + MinerU 文档解析引擎

版本：v1.2（2026-08-16）
状态：M0（MinerU PoC）实测完成，API 契约已冻结并回填 Rust 引擎；D1–D4 决策全部冻结
用途：提供给 AI Agent 直接进行架构分析、开发与重构

---

# 0. 变更摘要

## v1.2（相对 v1.1）—— M0 实测冻结

| # | 变更点 | v1.1 的说法 | v1.2 修订（M0 实测） | 来源 |
|---|--------|------------|----------------------|------|
| 1 | 任务状态值 | 排队/解析中/完成/失败/取消（大小写未定） | **小写** `pending` / `queued` / `processing` / `completed` / `failed` / `cancelled`；无逐页进度 | M0 实测 `GET /tasks/{id}` |
| 2 | 提交字段名 | 泛指 CLI 参数 `-b/-m/-f/-t` | **multipart 表单**：`files`（复数）、`parse_method`（非 `method`）、`formula_enable`、`table_enable`、`return_md` | M0 实测 `POST /tasks` + OpenAPI schema |
| 3 | 结果结构 | `{原文件名}.md + images/ + *_layout.pdf`（CLI 产物目录） | **API 模式无文件系统输出目录**：`GET /tasks/{id}/result` 内联返回 `{backend, version, results: {文件stem: {md_content, content_list}}}`；markdown 取自 `results[stem].md_content` | M0 实测 result 接口 |
| 4 | 并发上限 | 未知 | **3**（`max_concurrent_requests`）；超出时新任务 `queued` + `queued_ahead`，客户端须持续轮询 | M0 实测 `/health` |
| 5 | `.xls` 支持 | 未提及 | **400 不支持**（只认 `.xlsx`，openpyxl 驱动）；已从引擎 `SUPPORTED_EXTENSIONS` 剔除 | M0 实测提交 `.xls` |
| 6 | pipeline 依赖 | 未核实 | 需 `torch>=2.6` + torchvision（Windows CPU 版）+ `six`；docx/pptx 纯文本路径免 torch，PDF/图片必须 | M0 实测（缺 torch 报 `No module named 'torch'`） |
| 7 | Rust 引擎 | 未实现 | `mineru_engine.rs` 已按实测契约重写（小写状态、字段名、内联结果、`queued` 排队），`cargo check` 通过 | M0 收尾 |
| 8 | D1–D4 决策 | D1 已定，D2–D4 待 PoC 冻结 | **全部冻结**（D1/D4 已定并执行；D2/D3 标"候选待实测"） | 第 16 节 |
| 9 | 遗留项 | — | **取消语义**：mineru-api 无取消端点 → 客户端协作式取消（放弃轮询返回 Cancelled），已实现；**图片形式**：`return_images=true` 返回 data URI 映射，引擎内联替换（M1 已解决） | M1 收尾 |

---

## v1.1（相对 v1.0）

| # | 变更点 | v1.0 的说法 | v1.1 修订 | 来源 |
|---|--------|------------|-----------|------|
| 1 | MinerU 版本基线 | 未锁定 | **锁定 ≥ 3.4**（2026-06-18 发布） | 官方 GitHub README |
| 2 | 集成方式 | 方案 A/B/C 待评估，倾向"自写 Python 服务" | **直接托管官方 `mineru-api` 子进程**（FastAPI 服务，HTTP `/tasks` 异步接口），不写任何 Python 代码 | 官方架构文档 |
| 3 | 输出结构 | `document/ + assets/`（错误假设） | `{原文件名}.md + images/ + *_layout.pdf + *_span.pdf + *_middle.json + *_content_list.json + *_content_list_v2.json` | 官方文档实测 |
| 4 | 任务进度 | "处理第 24/32 页"（逐页进度，拿不到） | **任务级状态**：排队/解析中/完成/失败/取消；UI 只显示阶段 + 已用时长 | 官方 `/tasks` 接口 |
| 5 | 许可证 | "基于 Apache 2.0 + 附加条款" | **MinerU Open Source License**（3.1.0 起，Apache 2.0 + 附加条款），明确附加条款内容 | 官方 LICENSE |
| 6 | 硬件门槛 | 未核实 | **内存最低 16GB / 推荐 32GB+；纯 CPU 仅支持 pipeline 后端；GPU 需 Volta 及以上（pipeline 4GB、本地 VLM 8GB 显存）** | 官方文档 |
| 7 | 模型管理 | 泛指"下载模型" | `MINERU_MODEL_SOURCE=auto`（HF→ModelScope 自动选源 + 缓存）、`mineru-models-download` 预下载、离线 `=local`、`mineru.json` 配置 | 官方文档 |
| 8 | CLI/API 参数 | 泛指 backend/method | 锁定 `-b/--backend`、`-m/--method`、`--effort`、`-l/--lang`、`-s/-e`、`-f/--formula`、`-t/--table`、`--image-analysis` | 官方文档 |
| 9 | 决策点 | 未记录 | 新增 **D1–D4 决策记录**（D1 已定，D2–D4 待 PoC 冻结） | — |
| 10 | 开发顺序 | Phase 1–9 | 调整为 **里程碑 M0–M4**（M0 = PoC 优先） | — |

> M0 已实测冻结上述运行时行为（状态值、字段名、结果结构、并发、服务进程管理），详见第 17 节。唯一遗留：**取消语义**（API 取消 vs 杀进程），留 M1 任务闭环验证。

---

# 1. 产品重新定位

OmniMD 不再把自己定位成：

> "又一个 PDF / Office 转 Markdown 工具"

也不与 MinerU 竞争底层文档解析能力。

新的产品定位：

> **一个精致、极简、本地、免费的 Markdown 工作台。**

一句话：

> **Drop anything. Get clean Markdown.**

中文：

> **拖入任何文件，得到干净的 Markdown。**

核心理念：

```text
任何内容
  ↓
Import
  ↓
MinerU（唯一解析引擎）
  ↓
高质量 Markdown
  ↓
编辑 / 阅读 / 搜索 / 收藏 / 管理
  ↓
本地 Markdown 知识库
```

转换只是入口。

长期价值来自：

> **Markdown 文件管理 + 阅读 + 编辑 + 搜索 + 本地知识库工作流。**

---

# 2. 产品价值

OmniMD 的价值不是：

> "我的解析算法比 MinerU 更强。"

而是：

> **"我把专业文档解析能力变成一个普通人愿意每天使用的桌面软件。"**

## 目标用户

### A. AI 用户
经常把 PDF、Word、网页、论文交给 ChatGPT / Claude / NotebookLM。

### B. 知识管理用户
使用 Markdown、Obsidian、Logseq 等本地知识库。

### C. 办公用户
需要把 Word / PDF / PPT / Excel 转成 Markdown。

### D. 开发者 / AI Agent 用户
需要 Markdown 文档进入 Git、RAG、Cursor、Claude Code、Agent 工作流。

---

# 3. 产品核心原则

## 3.1 极简

用户不应该理解：

- MinerU、OCR、VLM、Pipeline、Hybrid、Model、Runtime

用户只需要理解：

> **导入 → Markdown → 使用**

## 3.2 本地优先

本地文件默认：

- 在本机处理、不上传、不需要登录、不需要 API Key

## 3.3 免费

基础核心功能完全免费。

## 3.4 Markdown 是最终数据格式

OmniMD 不把用户锁在自己的数据库里。用户最终得到的是真实 `.md` 文件（结构见第 10 节），文件属于用户自己。

## 3.5 文件优先

不要设计成必须注册账户、同步云端的 SaaS。本地文件夹就是知识库。

---

# 4. 产品结构

整体分为 3 层：

## 第一层：Import
> Anything → Markdown

支持：单文件、多文件、文件夹、URL、剪贴板内容（后续）。

## 第二层：Workspace
> Markdown → 阅读 / 编辑 / 搜索 / 收藏 / 管理

## 第三层：Knowledge
> Markdown → 本地知识库

未来可增加：AI 总结、AI 问答、RAG、MCP、Agent、ChatGPT/Claude 工作流。

第一阶段不要急着做第三层高级 AI。

---

# 5. MinerU 官方能力基线（2026-08 核实）

以下事实必须作为一切技术决策的输入，禁止使用二手博客推断。

## 5.1 版本

- 当前最新版本：**3.4**（2026-06-18 发布）。
- 注意：官方 changelog 文档页更新滞后于 GitHub README，**以 GitHub 仓库为准**。
- OmniMD 锁定 **MinerU ≥ 3.4**，安装时固定版本号，禁止 `latest` 漂移。

## 5.2 原生架构

MinerU 3.x 原生采用 **编排客户端 + FastAPI 服务** 架构：

- CLI 不带 `--api-url` 时，自动拉起本地临时 `mineru-api` 进程；
- CLI 带 `--api-url` 时，直连外部服务；
- `mineru-api` 是官方提供的独立服务进程（FastAPI）。

**这正是 OmniMD 需要的形态：无需自写 Python 服务，直接托管官方 `mineru-api` 子进程。**

## 5.3 mineru-api 接口（v1.2 契约，M0 实测）

| 接口 | 说明 |
|------|------|
| `GET /health` | 健康检查 |
| `POST /tasks` | 创建解析任务（异步，返回 `task_id`） |
| `GET /tasks/{id}` | 查询任务状态（任务级） |
| `GET /tasks/{id}/result` | 获取任务结果 |
| `POST /file_parse` | 同步解析（单次调用直接返回） |

- 任务结果默认保留 **24 小时**，可配置 `MINERU_API_TASK_RETENTION_SECONDS`（该项未实测，标注待验证）。
- 请求体为 **multipart 表单**（M0 实测）：`files`（复数，可多文件）、`parse_method`（`auto`/`txt`/`ocr`）、`formula_enable`、`table_enable`、`return_md`；提交响应含 `task_id` / `status` / `status_url` / `result_url` / `queued_ahead` / `message`。完整契约见第 17 节。

## 5.4 CLI / 请求参数（OmniMD 需要映射的参数全集）

| 参数 | 取值 | 说明 |
|------|------|------|
| `-b / --backend` | `pipeline` \| `vlm-engine` \| `hybrid-engine` \| `vlm-http-client` \| `hybrid-http-client` | 默认 `hybrid-engine` |
| `-m / --method` | `auto` \| `txt` \| `ocr` | 默认 `auto` |
| `--effort` | `medium` \| `high` | 推理投入度 |
| `-l / --lang` | 语言代码 | 默认中文环境 `zh` |
| `-s / -e` | 页码 | 页码范围 |
| `-f / --formula` | — | 启用公式识别 |
| `-t / --table` | — | 启用表格识别 |
| `--image-analysis` | — | 图片内容分析 |

> **API 字段名映射（M0 实测）**：上表为 CLI 参数；API 模式下实测对应字段为 `files` / `parse_method` / `formula_enable` / `table_enable` / `return_md`（`--effort`、`-l/--lang`、`-s/-e` 等字段 API 模式是否支持未实测，留 M1 验证）。

## 5.5 硬件门槛（D3 决策的直接输入）

- 内存：**最低 16GB，推荐 32GB+**（官方明确）。
- 纯 CPU：**仅支持 `pipeline` 后端**（vlm-engine / hybrid-engine 需要 GPU）。
- GPU：NVIDIA Volta 及以后；pipeline 约 4GB 显存，本地 VLM 约 8GB 显存。
- 磁盘：模型缓存占用在 M0 实测记录（pipeline 模型为 GB 级，VLM 更大）。

## 5.6 模型管理机制

- 默认 `MINERU_MODEL_SOURCE=auto`：HuggingFace → ModelScope 自动选源，并做缓存复用。
- 预下载命令：`mineru-models-download`（可提前把模型拉到本地）。
- 离线模式：`MINERU_MODEL_SOURCE=local`，读取本地预置模型。
- 用户级配置文件：`mineru.json`（在用户主目录）。

---

# 6. 核心技术架构（v1.2）

```text
                         OmniMD（Tauri 2）
                              │
              ┌───────────────┴───────────────┐
              │                                │
       Rust Core                            React UI
              │                                │
  ┌───────────┼──────────────┐                 │
  │           │              │                 │
Task Manager / Queue     DocumentEngine     Workspace
  │                       trait (HTTP)     SQLite + FTS5
  │           ┌────────────┴─────────┐          │
  │           │                      │      Library / Editor /
  │      MinerUEngine           AnyDocEngine   Preview / Search
  │        （HTTP client）        （未来）
  │           │
  │           └──── MinerU Runtime（独立子进程，崩溃隔离）
  │                     └── mineru-api（FastAPI, 127.0.0.1:<port>）
  │                             └── MinerU 后端 + 模型缓存
  │
  └─── Postprocess（输出整理 / metadata 提取 / 归入工作区）
                   ↓
            Markdown + images
                   ↓
             Workspace / 文件系统
```

关键点：

1. **OmniMD 不写任何 Python 代码**，不打包自定义服务，直接托管官方 `mineru-api`。
2. `mineru-api` 以子进程方式常驻（生命周期由 Rust 侧管理），满足崩溃隔离。
3. UI 与 MinerU 完全解耦：React 只跟 Task Manager 通信，Task Manager 只跟 `DocumentEngine` trait 通信。
4. 输出落到文件系统后，由 Postprocess 提取元数据（字数/图片数/表格数/来源），写入 SQLite 供 Workspace 使用。

---

# 7. MinerU 接入方式（D1：已定稿）

## 方案：托管官方 mineru-api 子进程（最终采用）

```text
Tauri (Rust)
   │  启动/停止/健康检查
   ↓
mineru-api --host 127.0.0.1 --port <port>   ← 官方 FastAPI 服务
   │
   └── MinerU 引擎 + 模型（单次加载，服务常驻复用）
```

### 生命周期

- 首次需要解析时由 Rust 侧启动（或安装时预启动）；
- 启动后 `GET /health` 轮询直到 ready（记录"首次模型加载时间"）；
- 任务期间保持常驻，避免重复加载模型；
- 退出应用时优雅关闭；崩溃时 Rust 捕获退出码并自动重启（带退避）。

### 优点

- 崩溃隔离（MinerU 崩了主程序不崩）；
- 依赖隔离（全部 Python 依赖在独立 venv/目录内）；
- 模型单次加载、后续任务复用（满足验收标准"模型不会重复加载"）；
- 版本升级 = 替换运行时目录；
- 不需要用户安装 Python（随包分发或首次安装引导）。

### 待 M0 实测确认的点

1. 取消任务语义：`POST /tasks` 是否提供取消端点？若无，取消 = 终止子进程（需确认对并发任务的影响，必要时"取消 = 杀进程 + 服务重启"）。
2. 并发：单服务进程是否支持并发任务？批量场景是否需要排队到任务级串行。
3. 端口与多实例：重复启动时端口占用处理。

> 结论：**只有 M0 实测推翻以下假设时，才回退到 CLI 子进程模式（每任务一个 `mineru` CLI 进程），CLI 模式为降级备选。**

---

# 8. 输出结构契约（v1.2 修正）

## MinerU 3.x 实际输出（必须以此为准）

> **模式区分（M0 实测）**：以下产物目录为 **CLI 模式**（`mineru -p -o {output}`）。
> **本产品采用 API 模式（托管 `mineru-api`），M0 实测无文件系统输出目录**，结果由 `GET /tasks/{id}/result` **内联返回**：
> ```json
> { "backend": "pipeline", "version": "3.4.5", "results": {
>   "文件stem": { "md_content": "...markdown 文本...", "content_list": [ ... ] }
> } }
> ```
> - 主交付物 markdown 文本取自 `results[文件stem].md_content`；
> - 元数据（字数、图片数、表格数、标题结构）取自 `content_list`；
> - API 模式下图片资源的具体形式（内联 data URI / 相对路径下载）M0 未实测，留 M1 验证；
> - 引擎实现：`src/engine/mineru_engine.rs`（已按此契约对齐）。

对输入 `{原文件名}.pdf`，**CLI 模式**输出目录内产生：

```text
{output}/
├── {原文件名}.md                      ← 主 Markdown（产品核心交付物）
├── images/                            ← 图片资源（SHA-1 哈希命名）
├── {原文件名}_layout.pdf              ← 版面分析可视化
├── {原文件名}_span.pdf                ← Span 可视化
├── {原文件名}_middle.json             ← 结构化中间数据（二次开发数据源）
├── {原文件名}_content_list.json       ← 内容列表
└── {原文件名}_content_list_v2.json    ← 内容列表 v2
```

## OmniMD 的输出约定

```text
{知识库}/{目录结构}/{原文件名}.md
{知识库}/{目录结构}/assets/{原文件名}/images/...
```

要求：

- 用户可见的主交付物是 `{原文件名}.md`；
- `images/` 中的图片路径在 `.md` 内为相对路径（PoC 验证相对关系后固化）；
- `_layout.pdf` / `_span.pdf` 是诊断副产品，**默认不展示、不打扰用户**，可在文档详情中折叠查看；
- `_middle.json` / `_content_list_v2.json` 是 OmniMD 的元数据来源（字数、图片数、表格数、标题结构），**但用户不需要理解它们**；
- 批量转换不覆盖其他文档；文件名安全；防止目录穿越；同名文件自动处理。

## 对 Workspace 的意义

- Document Info（标题/字数/图片/表格/来源类型）优先从 `_content_list_v2.json` / `_middle.json` 提取，而不是重新扫描解析 `.md`。
- 后续"在 Markdown 中定位图片""按标题跳转"可复用 `_content_list_v2.json` 的结构信息。

---

# 9. 任务状态与进度（v1.2 修正）

## 事实约束

MinerU API 提供的是**任务级状态**（`GET /tasks/{id}`），不提供逐页进度。M0 实测状态值为**小写**：`pending` / `queued` / `processing` / `completed` / `failed` / `cancelled`；`queued` 表示服务端并发（上限 3）排队中，客户端须持续轮询等待。

## OmniMD UI 进度模型

```text
排队中（pending/queued）→ 解析中（processing）→ 完成（completed）
                    └─────────→ 失败（failed）/ 取消（cancelled）
```

- 长任务显示"解析中" + **已用时长**（如 00:42），不显示虚假的页级进度；
- 批量任务显示：排队 N / 解析中 M / 完成 K / 失败 F；
- API 模式结果内联返回、无文件写盘，**不存在**"模型加载 / 生成输出"阶段信号；保持任务级进度，不升级阶段化进度条。

---

# 10. 解析模式（UI 三层映射）

普通用户不看到技术名词。UI 只提供：

```text
解析质量
● 自动
○ 快速
○ 高质量
```

内部映射（D3 耗时实测后冻结最终表；M0 benchmark 已取消，当前为候选）：

| UI | backend | method | 说明 |
|----|---------|--------|------|
| 自动 | `hybrid-engine`（无 GPU 时自动降级 `pipeline`） | `auto` | 默认路线，程序替用户做决策 |
| 快速 | `pipeline` | `auto` | 纯 CPU 唯一可用后端；速度优先 |
| 高质量 | `vlm-engine` 或 `hybrid-engine` | `auto` / `ocr` | 需要 GPU；无 GPU 时置灰并提示 |

- `--effort`、`-f/--formula`、`-t/--table`、`--image-analysis` 默认按官方推荐开启（表格/公式默认开），不暴露给普通用户；高级设置页可开关。
- 禁止让用户手动选择 OCR / TXT / VLM / Hybrid / Pipeline。

---

# 11. 默认解析策略

默认：**自动**。

系统根据：

- 文件类型；
- 是否存在文本层（文本 PDF 不需要 OCR）；
- 文档复杂度；
- 用户机器能力（有无 GPU、内存大小）。

自动选择合理路线。目标：**程序替用户做技术决策。**

---

# 12. 硬件门槛与性能基线（D3 输入）

## 官方硬件门槛（不可违背）

- 内存 < 16GB：**直接不展示"高质量/OCR"类选项**，仅允许 pipeline 快速模式；
- 内存 16–32GB：全部功能可用，提示推荐 32GB+；
- 无 NVIDIA Volta+ GPU：vlm-engine / hybrid-engine 不可用，UI 置灰。

## 启动性能目标（OmniMD 自身）

- 冷启动（应用本体）：≤ 2s；
- 首次模型加载（mineru-api ready）：M0 实测，目标 ≤ 60s（CPU）并展示明确进度；
- 后续转换：复用已加载模型，任务级速度由 M0 benchmark 决定展示文案（如"预计耗时"）。

## 安装包目标

- 基础安装包（不含模型）：目标 ≤ 200MB（M0 实测 mineru-api + Python 运行时实际体积后修正）；
- 模型：按分发策略（见第 14 节）单独管理。

---

# 13. 模型管理与分发（D2：候选方案）

## 官方机制（已确认）

- `MINERU_MODEL_SOURCE=auto`（默认）：HF → ModelScope 自动选源，缓存复用；
- `mineru-models-download` 预下载；
- 离线：`MINERU_MODEL_SOURCE=local`；
- 配置：用户目录 `mineru.json`。

## 候选方案

### 方案 A：完整离线包
- 安装包内含 Python 运行时 + mineru-api + 全部模型；
- 优点：安装即用、完全离线；缺点：安装包体积大（GB 级）。

### 方案 B：基础包 + 首次模型下载
- 安装包只含运行时；首次使用时由 OmniMD 调 `mineru-models-download`（或 `MINERU_MODEL_SOURCE=auto`）下载，带进度条与断点续传；
- 优点：安装包小；缺点：首次使用需联网。

### 决策规则
- M0 实测各后端模型体积与安装包体积后定；
- 默认倾向 **B + 设置页提供"预下载"与"本地离线包导入"**（用户可先下载离线包再离线安装模型）；
- 设置页模型管理 UI：

```text
模型
基础模型（pipeline）    1.8 GB   [下载] [更新] [清理缓存]
高质量模型（VLM）       3.2 GB   [下载] [更新] [清理缓存]
```
（体积为示意，M0 实测修正；要求：显示大小、下载进度、可重试、校验完整性、不重复下载。）

---

# 14. 安装包策略

比较项 | 完整离线包 | 基础包 + 模型下载
-------|-----------|------------------
安装即用 | ✓ | ✗（首次需下载）
完全离线 | ✓ | 需预下载
安装包体积 | 大 | 小
升级模型 | 重新打包 | 可单独更新

第一阶段默认：**基础包 + 模型下载，提供离线模型包导入**。最终以真实用户体验和 M0 体积数据决定。

---

# 15. MinerU 许可证与合规（v1.2 更新）

## 已核实事实

- MinerU **≥ 3.1.0** 采用 **MinerU Open Source License**（基于 Apache License 2.0 + 附加条款）；
- 附加条款包括（以官方 LICENSE 原文为准，发布前逐条复核）：
  - 月活跃用户超过 1 亿 **或** 月总收入超过 2000 万美元时，需要取得单独商业许可；
  - 基于 MinerU 向第三方提供在线服务时，需在产品/服务界面或公开文档中**显著标明使用 MinerU**；
  - 未履行适用条件时许可可能自动终止。

## OmniMD 的合规立场

第一阶段**仅做本地桌面客户端，不提供基于 MinerU 的在线 SaaS**，处于附加条款的免费范围内。

但必须：

1. 保留 MinerU License 原文（`licenses/MINERU-LICENSE.md`）；
2. 保留 Apache 相关 NOTICE；
3. 建立第三方依赖清单（`licenses/THIRD-PARTY-NOTICES.md`）；
4. 单独核查模型权重许可证（`licenses/MODEL-LICENSES.md`，模型许可与代码许可相互独立）；
5. 发布前进行许可证复核。

> 不能把"MinerU 使用 Apache 2.0 + 附加条款"理解为 MinerU 全部依赖和模型都自动是 Apache 2.0。

## OSS 合规目录

```text
licenses/
  MINERU-LICENSE.md
  THIRD-PARTY-NOTICES.md
  MODEL-LICENSES.md
```

每项记录：名称、版本、来源、License、是否随安装包发布、是否修改、备注。

---

# 16. 决策点记录（D1–D4）

| 决策点 | 状态 | 结论（当前，v1.2 冻结） | 冻结依据 / 遗留 |
|--------|------|-------------|----------|
| D1 集成方式 | **已冻结** | 托管官方 `mineru-api` 子进程（HTTP `/tasks`）；实测并发 3、小写状态、内联结果 | M0 实测（第 17 节）；**取消语义**留 M1 任务闭环实测 |
| D2 模型分发 | 候选（待体积实测） | 基础包 + 首次下载 + 离线包导入 | M0 全量 benchmark 已取消，模型/运行时体积未实测；保持候选，M3 前补测 |
| D3 CPU 性能底线 | 候选（待耗时实测） | 纯 CPU 仅 pipeline 后端，16GB 内存门槛；实测 PDF 可正常解析（torch CPU 版 2.13.0） | 8 类样本耗时/内存未实测（benchmark 已取消）；UI 置灰逻辑先按官方硬件门槛实现 |
| D4 现有 OCR/anydoc 去留 | **已冻结（已执行）** | MinerU 为唯一引擎；实测覆盖 docx/pptx/xlsx/pdf/图片，**`.xls` 除外**（400 不支持）；anydoc/PP-OCR 代码已清理 | M0 实测覆盖 + M1 代码清理完成（src 无残留，`ocr_resources/ppocrv6` 已删） |

---

# 17. MinerU PoC（M0）—— 实测结论（已冻结）

> 2026-08-16 完成。基于 mineru-api 3.4.5（Windows x64 / CPU / pipeline 后端），
> 冒烟测试与针对性探测已冻结 API 契约；8 类样本全量 benchmark 按评审要求取消。
> 脚本与样本位于 `poc/mineru_poc/`（`benchmark.py` / `samples/` / `restart_api.py`）。

## 已实测冻结的契约

| # | 契约点 | 实测结论 | 状态 |
|---|--------|----------|------|
| 1 | 任务状态值 | 小写 `pending` / `queued` / `processing` / `completed` / `failed` / `cancelled` | ✅ 已回填引擎 |
| 2 | 提交字段 | multipart `files`（复数）、`parse_method`、`formula_enable`、`table_enable`、`return_md` | ✅ 已回填引擎 |
| 3 | 提交响应 | `{task_id, status: pending, status_url, result_url, queued_ahead, message}` | ✅ 已回填引擎 |
| 4 | 轮询响应 | `{task_id, status, error?}`；排队任务带 `queued_ahead` | ✅ 已回填引擎 |
| 5 | 结果结构 | `GET /tasks/{id}/result` → `{backend, version, results: {文件stem: {md_content, content_list}}}`，**内联 markdown，无文件系统输出目录** | ✅ 已回填引擎 |
| 6 | 并发上限 | `max_concurrent_requests: 3`；超出排队，客户端须持续轮询 | ✅ 引擎识别 `queued` |
| 7 | 支持类型 | docx/pptx/xlsx/pdf/图片走通；**`.xls` 400 不支持**（只认 `.xlsx`） | ✅ 已剔除 `.xls` |
| 8 | 依赖 | pipeline 需 torch≥2.6（实测装 2.13.0+cpu）+ torchvision + six；docx/pptx 纯文本路径免 torch | ✅ 已记录 |

## 遗留项

1. **取消语义（M1 已解决）**：实测 mineru-api **无取消端点**（openapi 仅 `/health`、`/tasks`、`GET /tasks/{id}`、`GET /tasks/{id}/result`、`/file_parse`）；采用**客户端协作式取消**——取消 = 放弃轮询并返回 `Cancelled`，服务端任务自然结束（无法主动中止）。已实现于 `mineru_engine.rs` 轮询循环。
2. **API 模式图片形式（M1 已解决）**：实测 `return_images=true` 时 `results[stem]` 增 `images` 字段：`{文件名: "data:image/...;base64,..."}`；`md_content` 中引用为 `images/<文件名>`。引擎已提交 `return_images=true` 并做 **data URI 内联替换**，markdown 单文件自包含（不依赖输出目录）。可选优化：量大时改落盘 `images/` 子目录。
3. **模型体积 / 磁盘占用**：D2 冻结前补测（`MINERU_MODEL_SOURCE` 缓存目录实测大小）。

## 样本清单（8 类，已备齐，全量 benchmark 已取消）

10 页 PDF / 100 页 PDF / 扫描 PDF / 复杂表格 PDF / DOCX / PPTX / XLSX / 图片（含公式表格）。

## 产出（本轮完成）

- API 契约实测记录（上表）；
- `POST /tasks` 请求体字段（OpenAPI schema 核对）；
- 依赖清单与缺失排查结论（torch/six）；
- D1–D4 冻结并更新本文档为 **v1.2**；
- Rust 引擎按契约回填（`mineru_engine.rs`），`cargo check` 通过。

> 取消：8 类样本全量 benchmark、耗时/峰值内存报告、离线模型模式（`MINERU_MODEL_SOURCE=local`）实测——按评审要求取消，D2/D3 相应标"候选待实测"。

---

# 18. 里程碑路线图（M0–M4）

## M0：MinerU PoC（必须先做）
- 搭建 `poc/mineru_poc/`：Python venv + `mineru-api` 启动脚本 + 8 类样本 + benchmark 模板；
- 冻结 D1–D4；
- 产出第 17 节要求的报告。

## M1：引擎层重构
- 实现 `DocumentEngine` trait + `MinerUEngine`（HTTP client）；
- Rust 侧 `MinerURuntime`（子进程托管：启动/健康检查/崩溃重启/优雅关闭）；
- 移除或冻结 anydoc / PP-OCR 相关代码（D4 结论）；
- Task Manager 接入新引擎：queue / progress（任务级）/ cancel / retry / failure。

## M2：工作台（核心长期价值）
- ✅ SQLite Workspace（history / favorites / workspace metadata / FTS5 索引）—— 数据层已完成，详见第 27 节；
- ✅ Library 三栏 UI（工作区管理 / 文件夹树+文档列表 / Preview 面板）—— 进入自动增量索引，支持收藏 / 最近打开 / 中文全文检索；
- Markdown 编辑器（编辑 / 预览 / 原始，Typora 风格）；
- ✅ 全局搜索（SQLite FTS5，中文 + 英文）—— 已接入 Library 搜索框；
- Document Info（从 `_content_list_v2.json` 提取元数据）。

## M3：导入流闭环
- 首页拖放 / 选择文件 / 文件夹递归导入；
- 转换结果自动归入当前工作区并立即打开；
- 批量任务中心（全部/处理中/完成/失败，暂停/继续/取消/重试）；
- 失败处理与真实错误原因展示；
- URL 导入（复用现有 web_extractor）。

## M4：Windows 与发布
- Windows 右键菜单（"使用 OmniMD 转换为 Markdown"，单文件/多文件/文件夹/后台任务）；
- 安装包（模型分发策略落地）；
- 性能优化（100 页 PDF、批量 100 文件、1000 Markdown Library）；
- OSS 合规目录补齐与发布前复核。

---

# 19. 首页与导航

## 首页

首页彻底从"转换器"升级为"工作台入口"。

主标题：

> 把任何文件变成 AI 可用的 Markdown

副标题：

> 本地处理 · 免费 · 无需登录

格式：PDF · Word · PowerPoint · Excel · 图片 · EPUB · 网页

## 核心拖放框

```text
┌─────────────────────────────────────────┐
│                                         │
│       拖入文件或文件夹到这里              │
│                                         │
│      PDF · DOCX · PPTX · XLSX           │
│      图片 · EPUB 等                      │
│                                         │
│             [ 选择文件 ]                 │
│                                         │
├─────────────────────────────────────────┤
│ 文件 / 文件夹                    粘贴 URL │
└─────────────────────────────────────────┘
```

要求：第一眼知道怎么用、不堆功能、不出现开发术语、留白充足、入口明显。

## 首页空状态

```text
最近
还没有转换记录
拖入你的第一个文件开始
```

有历史后自动展示最近任务。

## 导航

从"首页 / 转换 / 批处理 / 设置"简化为：

```text
首页
知识库
历史
设置
```

原因：单文件、多文件、文件夹本质都是导入/转换，系统自动识别（1 文件→单任务，多文件→批量，文件夹→递归），用户不需要先选择"转换还是批处理"。

---

# 20. 知识库 / Library

这是 OmniMD 与普通转换器最重要的区别。

## 左侧栏

```text
知识库
📁 AI
📁 产品
📁 论文
📁 工作
📁 读书
⭐ 收藏
🕘 最近
```

知识库本质就是本地文件夹（如 `D:\Knowledge`），用户拥有真正的文件。

## 主界面（三栏）

```text
┌────────────┬─────────────────────┬────────────────────┐
│ 文件夹     │ Markdown 文件列表    │ Preview / Editor   │
│ ⭐ 收藏    │ 📄 ChatGPT.md       │ # ChatGPT          │
│ 🕘 最近    │ 📄 Claude.md        │ ## Features        │
└────────────┴─────────────────────┴────────────────────┘
```

三栏均可收起，适配不同屏幕尺寸。

## 文件列表

像现代桌面软件而非资源管理器：

```text
📄 ChatGPT.md   今天 20:31 · 12 KB
📄 Claude.md    昨天 18:42 · 8 KB
📄 Agent.md     2026/08/12 · 22 KB
```

显示：文件名、修改时间、文件大小；可选：类型、收藏状态、来源。不显示过多技术参数。

## 收藏

每个 Markdown 有 ☆，点击变 ★；左侧 ⭐ 收藏 快速访问。

## 最近使用

显示文件名 + 最后打开时间，点击直接打开。只保存文件路径与时间，**不复制 Markdown 内容到数据库**。

---

# 21. 导入工作流（最核心 UX）

```text
拖入文件
↓
自动解析
↓
自动进入当前知识库
↓
Markdown
↓
立即打开
```

不要强迫用户：转换 → 保存 → 找到文件 → 再导入知识库。

## 导入方式

- 📄 文件
- 📁 文件夹（递归扫描、保持目录结构、输出对应目录结构的 `.md`）
- 🔗 URL（访问网络，必须提示"URL 导入需要访问网络；本地文件默认不会上传"）
- 📋 剪贴板（未来）

---

# 22. Assets 管理

- 详情面板允许查看图片与表格资源，点击图片预览；
- 图片来自 MinerU 输出的 `images/`（相对路径引用）；
- 未来支持"在 Markdown 中定位"；
- 不要让用户必须理解 assets 才能正常使用 OmniMD。

---

# 23. 文档详情（Document Info）

右上角 ⓘ 展开（技术信息默认折叠）：

```text
标题 / 来源 / 创建 / 修改 / 字数 / 图片数 / 表格数 / 来源类型 / 解析方式
```

数据来源：`_content_list_v2.json` / `_middle.json`（第 8 节），不重新解析 `.md`。

---

# 24. 转换结果页与批量任务

## 结果页

保留双栏，产品化：

```text
← 返回
📄 report.pdf
✓ 转换完成 · 8.2 秒
[ 编辑 ] [ 预览 ] [ 原始 ]
[ 复制 Markdown ] [ 保存 ] [ 打开文件夹 ]
```

## 任务状态

```text
正在处理：report.pdf
✓ 准备文件
● 解析中 · 已用时 00:42
○ 生成输出
```

（任务级进度，见第 9 节；不要显示虚假的"第 24/32 页"。）

## 批量任务中心（非一级导航，专门任务页/弹层）

```text
全部 12 / 处理中 2 / 已完成 8 / 失败 2
```

单个任务显示文件、阶段、进度条。支持：暂停、继续、取消、重试、重试失败项。

---

# 25. 失败处理

必须显示真实原因：

```text
转换失败
文件：example.pdf
原因：解析引擎无法读取该文件
建议：
- 检查文件是否损坏
- 尝试重新导入
- 尝试高质量模式
```

批量汇总：100 文件 → ✓ 96 成功 / ⚠ 2 有警告 / ✕ 2 失败，支持重试失败项。

---

# 26. Windows 右键菜单

MVP 之外最重要的差异化功能之一。

- 文件：`使用 OmniMD 转换为 Markdown`
- 文件夹：`使用 OmniMD 批量转换为 Markdown`
- 支持单文件/多文件/文件夹/后台任务/打开主窗口查看进度。

目标：**用户看到 PDF 时，无需主动打开 OmniMD。**

---

# 27. 工作区 / Workspace

- 用户创建"我的 AI 知识库"，实际对应一个本地目录（如 `D:\Knowledge\AI`）；
- OmniMD 只负责展示、搜索、编辑、导入、管理；
- 不把用户数据锁在特殊数据库里；
- 第一阶段支持简单多工作区切换，每个工作区对应本地目录。

## 本地存储原则

- Markdown 本体永远保存在用户自己的文件系统；
- 数据库只记录：最近文件、收藏、最近打开时间、搜索索引、工作区配置；
- 数据库损坏不影响 Markdown 正文。

## 数据库

> SQLite（`rusqlite` bundled 特性，编译期内置 SQLite + FTS5，Windows 免系统依赖）

用途：History、Favorites、Workspace metadata、Full-text search index（FTS5）。
**不要把 Markdown 正文作为唯一数据源存储在 SQLite，源文件始终是 `.md`。**

### 数据层（M2 已实现，2026-08）

- 数据库文件：`app_data_dir()/omnimd.db`（Windows 为 `%APPDATA%/<identifier>/omnimd.db`），进程级单例持有；
- 表：
  - `workspaces`：名称 / 路径（唯一）/ 创建时间 / 最近打开；
  - `documents`：工作区 / 路径 / 标题 / 大小 / mtime（纳秒）/ 收藏 / 来源 / 创建与打开时间，`UNIQUE(workspace_id, path)`；
  - `documents_fts`：FTS5 虚拟表（title / body / tags 三列），rowid 与 `documents.id` 对齐；
  - `app_settings`：活动工作区等键值。
- 中文检索：应用层 CJK bigram 分词（`支持中文` → `支持 持中 中文`）+ FTS5 `unicode61` tokenizer，2 字以上中文词可命中，中英文混合文本统一索引；
- 增量索引：`scan_workspace` 递归扫描 `.md`，按 mtime 增量新增/更新，文件消失自动清理索引；索引可随时重建，不影响 `.md` 正文；
- frontmatter 轻量解析（`title` / `source` / `tags`）用于标题、来源展示与标签检索；
- 搜索返回 `{document, snippet}`，snippet 以 `<mark>` 标记命中词（前端渲染时去除标签并清理 CJK 空格）；
- Tauri 命令：`list_workspaces` / `add_workspace` / `remove_workspace` / `get_active_workspace` / `set_active_workspace` / `scan_workspace` / `list_documents` / `list_subfolders` / `list_favorites` / `list_recent` / `set_document_favorite` / `record_document_open` / `search_documents`。

---

# 28. 编辑器与渲染

## 编辑器

MVP：编辑、保存、撤销、重做、自动保存（可选）、快捷键、标题、粗体、斜体、列表、链接、代码块。

暂时不做：多人协作、实时云同步、评论、版本管理系统。

## Markdown 渲染

Preview 必须正确处理：H1–H6、段落、列表、表格、引用、图片、链接、代码块、LaTeX/公式、行内代码。

MinerU 输出中的复杂表格、公式必须在 Preview 里尽可能保真。

---

# 29. 主题、快捷键、新建、AI

## 主题

Light / Dark / System。视觉方向：Linear + Finder + Typora。关键词：干净、精致、克制、现代、少按钮、大留白、高信息密度但不拥挤。不做极客终端风、AI 炫技风、彩色卡片堆砌、复杂 SaaS Dashboard。

## 快捷键

- `Ctrl/Cmd + O` 打开文件
- `Ctrl/Cmd + Shift + O` 打开文件夹
- `Ctrl/Cmd + S` 保存
- `Ctrl/Cmd + F` 文档内搜索
- `Ctrl/Cmd + Shift + F` 全局搜索
- `Ctrl/Cmd + P` 快速打开文档
- `Ctrl/Cmd + N` 新建 Markdown

## 新建 Markdown

用户不仅可导入文件，也可新建 Markdown 直接进入编辑器，保存到当前工作区。这样 OmniMD 才是真正的"Markdown 工作台"而不只是转换器。

## AI 功能

第一阶段不做 AI 聊天。未来可增加：摘要、自动标签、自动标题、Frontmatter、文档问答、选中文本改写。原则：**AI 是增强能力，不是基础能力。**

---

# 30. Obsidian 支持 / CLI / Folder Watch（P1/P2）

## Obsidian 支持（P1/P2）

输出本质是标准 Markdown + assets（第 8 节结构），可选 Frontmatter：

```yaml
---
title: Example
source: https://example.com
created: 2026-08-16
tags:
  - imported
---
```

不要引入不可迁移的专有格式。

## CLI（P2）

```bash
omnimd report.pdf
omnimd ./documents --recursive
```

CLI 必须复用 Conversion Service，不实现第二套解析逻辑。

## Folder Watch（P2）

监控目录自动转换；必须：可关闭、支持 debounce、防止循环、修改 Markdown 不触发反向转换。

---

# 31. 搜索实现

第一版要求：本地全文搜索。

- 技术选型：SQLite FTS5（维护成本最低、性能足够）；若 benchmark 证明不足再评估 Tantivy；
- 搜索范围：文件名、标题、正文、tags、metadata；
- 要求：快速、本地、不依赖服务器、支持中英文、支持模糊搜索；
- 索引策略：文件系统变更时增量更新；FTS 索引损坏可重建，不影响 `.md` 正文。

---

# 32. 性能目标

重点不是"解析引擎 benchmark 第一"，而是**整个桌面应用使用起来舒服**。

## 启动
冷启动 / 热启动（应用本体 ≤ 2s）。

## 转换
首次模型加载（M0 实测，展示明确进度）/ 后续转换（复用模型）/ 批量转换。

## Workspace
打开 100 / 1000 个 Markdown、全文搜索响应。

## Preview
大 Markdown、大量图片、大表格。

## 资源
CPU、RAM、磁盘、模型缓存。

---

# 33. 本地隐私

必须明确：**本地文件默认在本机处理。**

不得默认上传：文件、Markdown、图片、OCR 内容、路径、文件名。

URL 和未来用户主动启用的 AI 功能例外。

---

# 34. 错误与日志

日志记录：task id、file type、engine、start/end time、error、exit code、runtime state。

禁止默认记录：Markdown 全文、OCR 全文、用户文件内容。

日志目录应可打开；普通用户不看到大量 debug 信息。

---

# 35. MinerU 崩溃隔离

MinerU 子进程崩溃不能让 OmniMD 主界面一起崩溃：

```text
mineru-api 崩溃
   ↓
Rust 捕获退出码 / 连接失败
   ↓
Task Manager 标记任务失败
   ↓
自动重启服务（退避重试）
   ↓
UI 提示 + 重试入口
```

---

# 36. 取消任务

```text
用户点击取消
↓
标记任务取消（UI 立即响应）
↓
调用 API 取消接口（M0 验证）；若无则终止 mineru-api 子进程
↓
删除临时文件 / 清理任务结果
↓
状态 = cancelled
```

不能出现：UI 已显示取消但 MinerU 仍继续运行。若"取消 = 杀进程"成为既定语义，并发任务会一起终止，此时取消前 UI 提示影响范围（M0 后冻结交互文案）。

---

# 37. Fallback

第一阶段不强制 anydoc fallback。未来若加入，必须明确提示"⚠ 主解析方式失败，已使用备用解析方式"，不要静默改变输出来源。

---

# 38. 第一阶段 MVP

必须：

- MinerU 集成（mineru-api 托管）
- PDF / DOCX / PPTX / XLSX / 图片
- 单文件 / 多文件 / 文件夹
- Markdown 输出（真实 `.md` + images）
- Preview / 编辑 / 原始
- 本地工作区 / 最近 / 收藏 / 基础搜索 / 历史
- 错误处理与真实原因
- Windows 右键菜单
- 本地运行、不上传
- License / Third-party Notice

暂不做：云同步、账号、在线存储、团队协作、在线 SaaS、AI 聊天、RAG、MCP、CLI、Folder Watch、复杂 Obsidian 双向同步、Notion 同步、社交功能。

---

# 39. 验收标准

## 核心体验

- [ ] 用户第一次启动后能在 30 秒内完成第一次转换（含首次模型加载；若 M0 表明无法达成，则为"展示明确的加载进度"，不静默）
- [ ] 拖入文件即可开始
- [ ] 不需要理解 MinerU / 不需要 Python / 不需要命令行 / 不需要账号 / 不需要 API Key

## 转换

- [ ] PDF / DOCX / PPTX / XLSX / 图片 / 文件夹 / 批量
- [ ] Markdown + Assets（第 8 节契约）
- [ ] 无 GPU 时"高质量"选项正确置灰

## Workspace

- [ ] 新建 Markdown / 编辑 / Preview / 原始 Markdown / 搜索 / 收藏 / 最近 / 文件夹 / 工作区
- [ ] Document Info 从 `_content_list_v2.json` 正确提取

## Windows

- [ ] 右键菜单（单文件 / 多文件 / 文件夹 / 后台处理）

## 稳定性

- [ ] MinerU 崩溃不导致主程序崩溃
- [ ] 取消任务真实停止
- [ ] 大文件不明显卡死
- [ ] 长任务有进度（任务级 + 已用时长）
- [ ] 模型不会重复加载

## 合规

- [ ] MinerU License（MinerU Open Source License 原文）
- [ ] NOTICE / Third-party License / Model License

---

# 40. 明确禁止

AI Agent 不得：

1. 重新开发 OCR / PDF Layout Parser / 表格识别 / 公式识别。
2. 同时集成 MinerU + anydoc，除非 benchmark 明确证明必要（D4 默认移除）。
3. 直接把 MinerU 代码散落到 React，或让 UI 直接依赖 MinerU 内部 API。
4. 自写 Python 解析服务（应托管官方 `mineru-api`）。
5. 默认把所有模型塞进安装包而不评估体积。
6. 要求用户安装 Python。
7. 让 MinerU 崩溃导致主程序崩溃。
8. 把 MinerU 的工程术语暴露给普通用户。
9. 为了"功能丰富"把首页做成复杂 Dashboard。
10. 把 Markdown 作为专有数据库格式锁死。
11. 把第三方依赖许可证当成与 MinerU 主许可证相同。
12. 在未重新审查许可证的情况下做在线 MinerU SaaS。
13. 一开始就堆 AI、RAG、MCP 等高级功能。
14. 使用二手博客 / AI 推断作为 MinerU 行为依据（一切以官方文档与仓库为准，行为性事实以 M0 实测为准）。
15. 在 UI 上显示 MinerU 拿不到的逐页进度（只显示任务级状态）。

---

# 41. 产品视觉方向

> 精致、安静、现代、克制、本地、专业。

参考气质：Linear 的精致、Finder 的文件管理、Typora 的 Markdown 阅读、少量 Obsidian 的知识库体验。

不做：复杂 SaaS Dashboard、大量彩色卡片、AI 炫技动画、极客终端视觉、满屏按钮。

---

# 42. 最终产品形态

```text
┌───────────────────────────────────────────────────────────┐
│ OmniMD                              🔍 搜索      ⚙       │
├────────────┬────────────────────────┬─────────────────────┤
│ 📁 AI      │ 📄 ChatGPT.md         │ # ChatGPT           │
│ 📁 产品    │ 📄 Claude.md          │ ## Overview         │
│ 📁 论文    │ 📄 Agent.md           │ 正文……              │
│ ⭐ 收藏    │ 📄 MCP.md             │                     │
│ 🕘 最近    │                        │                     │
│ ＋ 新建    │                        │                     │
└────────────┴────────────────────────┴─────────────────────┘
```

用户拖入 `paper.pdf`：

```text
paper.pdf
↓
MinerU（mineru-api 子进程）
↓
paper.md + images
↓
自动加入当前 Workspace
↓
立即打开
```

长期：

```text
MinerU 负责"把复杂内容变成高质量 Markdown"
OmniMD 负责"让用户真正愿意使用这些 Markdown"
```

OmniMD 的竞争力不是解析引擎，而是：

> **更简单 + 更漂亮 + 更本地 + 更自由 + 更像一个真正的桌面工具。**

---

# 43. AI Agent 最终执行指令

你现在负责在现有 OmniMD 项目上，把产品逐步演进为：

> **基于 MinerU 文档解析能力的本地 Markdown 工作台。**

必须遵守：

1. 先分析现有项目（已完成 Phase 1 尽调），再修改。
2. 先做 M0 MinerU PoC，再决定/冻结 D1–D4。
3. 当前只实现 MinerU Engine；anydoc 仅保留架构扩展位，不默认集成。
4. 集成方式：托管官方 `mineru-api` 子进程，Tauri 走 HTTP 接口，不写 Python 服务代码。
5. UI 与 MinerU 完全解耦。
6. MinerU 独立进程运行，崩溃隔离。
7. 优先优化 Windows 安装、模型与 runtime 体积。
8. 转换不是唯一功能，Workspace 是核心长期价值。
9. Markdown 必须是真实 `.md` 文件，输出契约按第 8 节。
10. 本地文件默认不上传。
11. 不做账号和云同步。
12. 首页面向普通用户，不暴露底层技术；进度只显示任务级状态。
13. 第一阶段做 Import + Workspace + Preview + Search + History。
14. Windows 右键菜单是重要差异化能力。
15. 不要重复开发 OCR / Layout / Table Parser。
16. 所有第三方依赖和模型必须做许可证清单（MinerU Open Source License 原文 + Model License 单独核查）。
17. 所有技术决策优先通过真实 benchmark（M0），而不是凭感觉。
18. 不要因为"以后可能需要"提前塞入大量复杂依赖。
19. MinerU 版本锁定 ≥3.4，任何升级需回归验证。

最终验收不是：

> "OmniMD 的解析能力是否超过 MinerU？"

而是：

> **"普通用户是否更愿意使用 OmniMD 完成 文件 → Markdown → 阅读/编辑/管理 这一整套流程。"**
