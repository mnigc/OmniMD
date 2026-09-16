<div align="center">

[English](README.md) | [简体中文](README.zh.md) | [한국어](README.ko.md) | [日本語](README.ja.md)

---

# OmniMD

### Anything to Markdown

将 PDF、Word、Excel、PPT、EPUB、HTML 等文档一键转换为 Markdown 的跨平台桌面应用。

基于 **Tauri 2** + **Rust** + **React** 构建，核心转换引擎由 [AnyDoc](https://github.com/firecrawl/anydoc) 提供（本地纯 Rust 引擎，无需 ML 模型、无需联网）。

</div>

---

## ✨ 功能特性

- **多格式支持** — PDF、DOC/DOCX、PPT/PPTX、XLS/XLSX、EPUB、CSV、ODT/ODS/ODP、RTF 等 20+ 种格式
- **毫秒级转换** — 纯 Rust 进程内引擎，无需 Python、无需下载模型
- **批量转换** — 一次性拖入多个文件，并发转换，实时进度反馈
- **资源提取** — 自动提取文档内嵌图片，保存到每篇文档的 `assets/` 目录
- **智能格式识别** — 通过文件魔数（magic bytes）自动检测格式，不依赖扩展名
- **Markdown 工作台** — 知识库（文件夹树、中英文全文检索、收藏、最近）、预览与编辑
- **本地优先** — 全部转换在本地完成，文件不上传，隐私安全
- **桌面原生体验** — Tauri 打包为原生应用，体积小、启动快

## 📸 界面预览

> TODO: 添加截图

主要页面：

| 页面 | 功能 |
|------|------|
| **Home** | 拖入文件/文件夹，设置并发数与输出位置，跟踪转换队列 |
| **Library** | 工作区文件夹、文档列表、全文搜索、收藏、最近、预览与编辑 |
| **History** | 全部已完成 / 失败 / 取消的转换记录 |
| **Preview** | 单文档的源码 / 预览 / 分栏视图 |
| **Settings** | 主题、语言、默认输出目录 |

## 🚀 快速开始

### 环境要求

| 依赖 | 版本要求 | 说明 |
|------|---------|------|
| [Node.js](https://nodejs.org) | ≥ 18 (LTS) | 前端运行时 |
| [pnpm](https://pnpm.io) | ≥ 8 | 前端包管理器 |
| [Rust](https://www.rust-lang.org/tools/install) | stable | 后端编译 |
| MSVC Build Tools | — | Windows 编译必需（勾选"使用 C++ 的桌面开发"） |

### 安装与运行

```bash
# 1. 克隆仓库
git clone https://github.com/<your-org>/OmniMD.git
cd OmniMD

# 2. 安装后端（Tauri CLI）+ 前端依赖
pnpm install        # 安装根目录依赖（含 Tauri CLI）
cd frontend && pnpm install

# 3. 启动开发模式（同时编译 Rust 后端 + 前端热更新）
pnpm tauri dev
```

首次启动会编译大量 Rust 依赖，预计 5–15 分钟，之后增量编译很快。完成后会自动弹出一个桌面窗口。

## 📦 打包发布

```bash
pnpm bundle:tools   # 预装打包工具（WiX 等），避免构建时从 GitHub 下载超时
pnpm tauri build    # 打包生成安装包
```

产物位于 `target/release/bundle/`，Windows 下生成 `.msi` 安装包。

> 说明：`bundle:tools` 只需在出安装包、且本机尚未缓存 WiX 时跑一次。

### 开箱即用

应用启动即可直接转换文档，无需安装 Python、无需下载模型、无需联网。

## 🏗️ 项目结构

```
OmniMD/
├── src/                    # Rust 后端
│   ├── main.rs             # 程序入口
│   ├── lib.rs              # Tauri 命令注册（前端调用的接口）
│   ├── markdown_pipeline.rs# 后处理（标题/列表规范化、清理、统计）
│   ├── file_utils.rs       # 路径处理 / 格式白名单
│   ├── db/                 # SQLite 工作区数据层（元数据 + FTS5 检索）
│   ├── engine/             # DocumentEngine trait + AnyDoc 实现 + 批量队列
│   └── models/             # Document / Task / Asset 数据结构
├── frontend/               # React + TypeScript 前端
│   ├── src/
│   │   ├── App.tsx         # 应用骨架与导航
│   │   ├── pages/          # Home / Library / History / Preview / Settings 页面
│   │   ├── api/            # 调用 Rust 后端的 invoke 封装
│   │   ├── store/          # zustand 状态管理
│   │   ├── components/     # 复用组件
│   │   └── types/          # 共享类型定义
│   └── vite.config.ts      # Vite 配置（端口 1421）
├── tauri.conf.json         # Tauri 应用配置
├── capabilities/           # Tauri 权限配置
├── Cargo.toml              # Rust 依赖
└── tests/                  # 集成测试 + fixtures
```

## 🔌 前后端通信

前端通过 Tauri 的 `invoke` 调用 Rust 端注册的命令，接口定义见 [`src/lib.rs`](src/lib.rs)：

| 命令 | 入参 | 返回 | 说明 |
|------|------|------|------|
| `convert_file` | `sourcePath`, `outputDir` | `ConversionResult` | 转换单文件 |
| `cancel_task` | `taskId` | — | 协作式取消转换 |
| `get_supported_formats` | — | `string[]` | 支持的扩展名列表 |
| `batch_enqueue` / `batch_start` / `batch_cancel_all` … | 见 `src/lib.rs` | — | 批量队列控制 |
| `list_workspaces` / `scan_workspace` / `list_documents` / `search_documents` … | 见 `src/lib.rs` | — | 知识库数据层（SQLite + FTS5） |

转换过程中通过 Tauri 事件 `task-progress` / `task-status`（批量另有 `batch-progress` / `batch-status` / `batch-summary`）推送进度，前端可监听并实时更新 UI。

## 🧪 测试

```bash
# Rust 单元测试 + 集成测试
cargo test

# 前端类型检查 + 构建
cd frontend && pnpm build
```

测试 fixtures 位于 [`tests/fixtures/`](tests/)。

## 🛠️ 技术栈

| 层 | 技术 |
|----|------|
| 桌面框架 | Tauri 2 |
| 后端语言 | Rust 2021 edition |
| 转换引擎 | [AnyDoc](https://github.com/firecrawl/anydoc)（本地纯 Rust 引擎，MIT） |
| 异步运行时 | tokio |
| 前端框架 | React 18 + TypeScript 5 |
| 构建工具 | Vite 5 |
| 样式 | Tailwind CSS 3 |
| 状态管理 | zustand |
| Markdown 渲染 | react-markdown + remark-gfm |

## 📄 许可证

TODO: 添加 LICENSE 文件（建议 MIT 或 Apache-2.0）。

## 🗺️ 路线图

- [x] 单文件 / 批量转换（本地纯 Rust 引擎）
- [x] 知识库（工作区文件夹、全文检索、收藏、最近）
- [x] 预览 / 编辑（自动保存）
- [x] 转换历史
- [x] 主题（浅色 / 深色 / 跟随系统）与语言（中 / 英）切换
- [ ] Windows 右键菜单集成
- [ ] 跨平台构建（macOS / Linux）

## 📖 更多文档

详细的开发、调试、打包说明见 **[开发调试指南](docs/开发调试指南.md)**。

## 🤝 贡献

欢迎提 Issue 和 PR。提 PR 前请确保：

```bash
cargo test          # 后端测试通过
cd frontend && pnpm build   # 前端构建无报错
```

---

<div align="center">

Built with Tauri · Rust · React

</div>
