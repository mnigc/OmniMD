<div align="center">

[English](README.md) | [简体中文](README.zh.md) | [한국어](README.ko.md) | [日本語](README.ja.md)

---

# OmniMD

### Anything to Markdown

将 PDF、Word、Excel、PPT、EPUB、HTML 等文档一键转换为 Markdown 的跨平台桌面应用。

基于 **Tauri 2** + **Rust** + **React** 构建，核心转换引擎由 [MinerU](https://github.com/opendatalab/MinerU) 3.x 提供。

</div>

---

## ✨ 功能特性

- **多格式支持** — PDF、DOC/DOCX、PPT/PPTX、XLS/XLSX、EPUB、CSV、TXT、HTML、ODT/ODS/ODP、RTF 等 20+ 种格式
- **批量转换** — 一次性拖入多个文件，并发转换，实时进度反馈
- **资源提取** — 自动提取文档中的图片等资源，保存到 `assets/` 目录
- **智能格式识别** — 通过文件魔数（magic bytes）自动检测格式，不依赖扩展名
- **文本直通** — 纯文本、JSON、XML、HTML 等直接透传为 Markdown 代码块/段落
- **本地优先** — 全部转换在本地完成，文件不上传，隐私安全
- **桌面原生体验** — Tauri 打包为原生应用，体积小、启动快

## 📸 界面预览

> TODO: 添加截图

应用包含三个主页面：

| 页面 | 功能 |
|------|------|
| **Home** | 文件浏览与快速入口 |
| **Convert** | 单文件转换，支持预览输出 |
| **Batch** | 批量转换，并发控制与进度追踪 |

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

发布安装包前，先把**便携版 Python + mineru** 预装进包体（一次构建、用户端零配置）：

```bash
# 1. 预装打包工具（WiX 等），避免构建时从 GitHub 下载超时
pnpm bundle:tools

# 2. 预装 Python 运行环境与 mineru 到 bundle_extras/python/
pnpm bundle:python

# 3. 打包（会自动把 bundle_extras/python 打进安装包）
pnpm tauri build
```

产物位于 `target/release/bundle/`，Windows 下生成 `.msi` 安装包。

> 说明：`bundle_extras/` 由构建脚本生成（多 GB，已加入 `.gitignore`），**不提交到仓库**。普通开发（`pnpm tauri dev`）无需此步骤——缺失内置 Python 时应用会在首次启动时自动下载安装，行为与之前一致。`bundle:tools` 只需在出安装包、且本机尚未缓存 WiX 时跑一次。

### 开箱即用（免手动配置）

安装后应用会自动完成环境就绪：首次启动静默下载默认 pipeline 模型并拉起 MinerU 引擎，**用户无需在设置里手动安装 Python 运行环境或点击下载模型**即可直接转换文档。若首次启动无网络导致准备失败，界面会提示重试，也可在「设置 → 模型管理」导入离线模型。

## 🏗️ 项目结构

```
OmniMD/
├── src/                    # Rust 后端
│   ├── main.rs             # 程序入口
│   ├── lib.rs              # Tauri 命令注册（前端调用的接口）
│   ├── pipeline.rs         # 转换流水线（读取 → 转换 → 写出）
│   ├── file_utils.rs       # 路径处理 / 格式白名单
│   ├── converters/         # MinerU 引擎集成（HTTP client + 运行时管理）
│   └── models/             # Document / Task / Asset 数据结构
├── frontend/               # React + TypeScript 前端
│   ├── src/
│   │   ├── App.tsx         # 应用骨架与导航
│   │   ├── pages/          # Home / Convert / Batch 页面
│   │   ├── api/            # 调用 Rust 后端的 invoke 封装
│   │   ├── store/          # zustand 状态管理
│   │   ├── components/     # 复用组件
│   │   └── types/          # 共享类型定义
│   └── vite.config.ts      # Vite 配置（端口 1420）
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
| `get_supported_formats` | — | `string[]` | 支持的扩展名列表 |
| `get_converter_info` | — | `ConverterInfo` | 转换器名称与支持格式 |

转换过程中通过 Tauri 事件 `task-progress` / `task-status` 推送进度，前端可监听并实时更新 UI。

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
| 转换引擎 | [MinerU](https://github.com/opendatalab/MinerU) 3.x (via `mineru-api`) |
| 异步运行时 | tokio |
| 前端框架 | React 18 + TypeScript 5 |
| 构建工具 | Vite 5 |
| 样式 | Tailwind CSS 3 |
| 状态管理 | zustand |
| Markdown 渲染 | react-markdown + remark-gfm |

## 📄 许可证

TODO: 添加 LICENSE 文件（建议 MIT 或 Apache-2.0）。

## 🗺️ 路线图

- [x] Phase 1 MVP — 单文件 / 批量转换、MinerU 引擎集成
- [ ] Settings 页面配置（输出格式、并发数、OCR 开关）
- [ ] OCR 图片文字识别（模型结构已预留于 `src/models/ocr.rs`）
- [ ] 拖拽文件夹递归转换
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
