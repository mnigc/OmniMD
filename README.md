<div align="center">

[English](README.md) | [简体中文](README.zh.md) | [한국어](README.ko.md) | [日本語](README.ja.md)

---

# OmniMD

### Anything to Markdown

A cross-platform desktop application that converts documents such as PDF, Word, Excel, PowerPoint, EPUB, and HTML into clean Markdown.

Built with **Tauri 2** + **Rust** + **React**. Core conversion powered by [AnyDoc](https://github.com/firecrawl/anydoc) — a local, pure-Rust engine with no ML models and no network access.

</div>

---

## ✨ Features

- **20+ format support** — PDF, DOC/DOCX, PPT/PPTX, XLS/XLSX, EPUB, CSV, ODT/ODS/ODP, RTF and more
- **Millisecond conversion** — Pure-Rust, in-process engine; no Python, no models to download
- **Batch conversion** — Drag in multiple files at once, convert concurrently with real-time progress
- **Asset extraction** — Automatically extracts embedded images into a per-document `assets/` directory
- **Smart format detection** — Detects format via magic bytes, independent of file extension
- **Markdown workbench** — Library with folder tree, full-text search (Chinese + English), favorites, recent, preview and edit
- **Local-first** — All conversion happens locally; no files are uploaded, ensuring privacy
- **Native desktop experience** — Tauri packages a small, fast native app

## 📸 Screenshots

> TODO: Add screenshots

Main pages:

| Page | Purpose |
|------|---------|
| **Home** | Drop files / folders, configure concurrency and output location, track the conversion queue |
| **Library** | Workspace folders, documents, full-text search, favorites, recent, preview & edit |
| **History** | Every completed / failed / cancelled conversion |
| **Preview** | Single-document source / preview / split view |
| **Settings** | Theme, language, default output directory |

## 🚀 Getting Started

### Prerequisites

| Dependency | Version | Notes |
|------------|---------|-------|
| [Node.js](https://nodejs.org) | ≥ 18 (LTS) | Frontend runtime |
| [pnpm](https://pnpm.io) | ≥ 8 | Package manager |
| [Rust](https://www.rust-lang.org/tools/install) | stable | Backend compiler |
| MSVC Build Tools | — | Required on Windows (select "Desktop development with C++") |

### Install & Run

```bash
# 1. Clone the repo
git clone https://github.com/<your-org>/OmniMD.git
cd OmniMD

# 2. Install backend (Tauri CLI) + frontend dependencies
pnpm install        # installs root deps (Tauri CLI)
cd frontend && pnpm install

# 3. Start dev mode (compiles Rust backend + frontend hot-reload)
pnpm tauri dev
```

The first launch compiles many Rust dependencies (expect 5–15 min). Subsequent incremental builds are fast. A desktop window appears automatically when ready.

## 📦 Build for Release

```bash
pnpm tauri build
```

Artifacts are placed in `target/release/bundle/` (`.msi` / `.exe` installers on Windows).

## 🏗️ Project Structure

```
OmniMD/
├── package.json              # Root scripts + Tauri CLI
├── Cargo.toml                # Rust dependencies
├── tauri.conf.json           # Tauri app configuration
├── capabilities/             # Tauri permission configuration
├── icons/                    # Application icons
├── src/                      # Rust backend
│   ├── main.rs             # Entry point
│   ├── lib.rs              # Tauri command registration (frontend API)
│   ├── markdown_pipeline.rs# Post-processing (heading/list normalization, cleanup, stats)
│   ├── file_utils.rs       # Path helpers / format whitelist
│   ├── db/                 # SQLite workspace layer (metadata + FTS5 search)
│   ├── engine/             # DocumentEngine trait + AnyDoc implementation + batch queue
│   └── models/             # Document / Task / Asset data structures
├── frontend/               # React + TypeScript frontend
│   ├── src/
│   │   ├── App.tsx         # App shell and navigation
│   │   ├── pages/          # Home / Library / History / Preview / Settings
│   │   ├── api/            # invoke wrappers for Rust backend
│   │   ├── store/          # zustand state management
│   │   ├── components/     # Reusable components
│   │   └── types/          # Shared type definitions
│   └── vite.config.ts      # Vite config (port 1421)
```

## 🔌 Frontend–Backend Communication

The frontend calls registered Rust commands via Tauri's `invoke`. See [`src/lib.rs`](src/lib.rs):

| Command | Parameters | Returns | Description |
|---------|-----------|---------|-------------|
| `convert_file` | `sourcePath`, `outputDir` | `ConversionResult` | Convert a single file |
| `cancel_task` | `taskId` | — | Cooperatively cancel a conversion |
| `get_supported_formats` | — | `string[]` | Supported extensions |
| `batch_enqueue` / `batch_start` / `batch_cancel_all` … | see `src/lib.rs` | — | Batch queue control |
| `list_workspaces` / `scan_workspace` / `list_documents` / `search_documents` … | see `src/lib.rs` | — | Library data layer (SQLite + FTS5) |

During conversion, progress is pushed via Tauri events `task-progress` / `task-status` (and `batch-progress` / `batch-status` / `batch-summary`), which the frontend listens to for live UI updates.

## 🧪 Testing

```bash
# Rust unit + integration tests
cargo test

# Frontend type-check + build
cd frontend && pnpm build
```

Test fixtures live in [`tests/fixtures/`](tests/).

## 🛠️ Tech Stack

| Layer | Technology |
|-------|-----------|
| Desktop framework | Tauri 2 |
| Backend language | Rust 2021 edition |
| Conversion engine | [AnyDoc](https://github.com/firecrawl/anydoc) (local pure-Rust engine, MIT) |
| Async runtime | tokio |
| Frontend framework | React 18 + TypeScript 5 |
| Build tool | Vite 5 |
| Styling | Tailwind CSS 3 |
| State management | zustand |
| Markdown rendering | react-markdown + remark-gfm |

## 📄 License

TODO: Add a LICENSE file (MIT or Apache-2.0 recommended).

## 🗺️ Roadmap

- [x] Single-file / batch conversion with a local pure-Rust engine
- [x] Library (workspace folders, full-text search, favorites, recent)
- [x] Preview / edit with autosave
- [x] Conversion history
- [x] Theme (light / dark / system) and language (zh / en) switching
- [ ] Windows shell context-menu integration
- [ ] Cross-platform builds (macOS / Linux)

## 📖 More Documentation

Detailed development, debugging, and packaging instructions in **[Development & Debugging Guide](docs/开发调试指南.md)** (in Chinese).

## 🤝 Contributing

Issues and PRs are welcome. Before submitting a PR, please ensure:

```bash
cargo test                    # Backend tests pass
cd frontend && pnpm build     # Frontend builds without errors
```

---

<div align="center">

Built with Tauri · Rust · React

</div>
