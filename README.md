<div align="center">

[English](README.md) | [简体中文](README.zh.md) | [한국어](README.ko.md) | [日本語](README.ja.md)

---

# OmniMD

### Anything to Markdown

A cross-platform desktop application that converts documents such as PDF, Word, Excel, PowerPoint, EPUB, and HTML into clean Markdown.

Built with **Tauri 2** + **Rust** + **React**. Core conversion powered by [MinerU](https://github.com/opendatalab/MinerU) 3.x.

</div>

---

## ✨ Features

- **20+ format support** — PDF, DOC/DOCX, PPT/PPTX, XLS/XLSX, EPUB, CSV, TXT, HTML, ODT/ODS/ODP, RTF and more
- **Batch conversion** — Drag in multiple files at once, convert concurrently with real-time progress
- **Asset extraction** — Automatically extracts images and other resources into an `assets/` directory
- **Smart format detection** — Detects format via magic bytes, independent of file extension
- **Text passthrough** — Plain text, JSON, XML, and HTML pass through directly as Markdown blocks
- **Local-first** — All conversion happens locally; no files are uploaded, ensuring privacy
- **Native desktop experience** — Tauri packages a small, fast native app

## 📸 Screenshots

> TODO: Add screenshots

The app has three main pages:

| Page | Purpose |
|------|---------|
| **Home** | File browsing and quick access |
| **Convert** | Single-file conversion with output preview |
| **Batch** | Batch conversion with concurrency control and progress tracking |

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
│   ├── pipeline.rs         # Conversion pipeline (read → convert → write)
│   ├── file_utils.rs       # Path helpers / format whitelist
│   ├── converters/         # MinerU engine integration (HTTP client + runtime management)
│   └── models/             # Document / Task / Asset data structures
├── frontend/               # React + TypeScript frontend
│   ├── src/
│   │   ├── App.tsx         # App shell and navigation
│   │   ├── pages/          # Home / Convert / Batch pages
│   │   ├── api/            # invoke wrappers for Rust backend
│   │   ├── store/          # zustand state management
│   │   ├── components/     # Reusable components
│   │   └── types/          # Shared type definitions
│   └── vite.config.ts      # Vite config (port 1420)
├── tests/                  # Integration tests + fixtures
```

## 🔌 Frontend–Backend Communication

The frontend calls registered Rust commands via Tauri's `invoke`. See [`src/lib.rs`](src/lib.rs):

| Command | Parameters | Returns | Description |
|---------|-----------|---------|-------------|
| `convert_file` | `sourcePath`, `outputDir` | `ConversionResult` | Convert a single file |
| `get_supported_formats` | — | `string[]` | Supported extensions |
| `get_converter_info` | — | `ConverterInfo` | Converter name and formats |

During conversion, progress is pushed via Tauri events `task-progress` / `task-status`, which the frontend listens to for live UI updates.

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
| Conversion engine | [MinerU](https://github.com/opendatalab/MinerU) 3.x (via `mineru-api`) |
| Async runtime | tokio |
| Frontend framework | React 18 + TypeScript 5 |
| Build tool | Vite 5 |
| Styling | Tailwind CSS 3 |
| State management | zustand |
| Markdown rendering | react-markdown + remark-gfm |

## 📄 License

TODO: Add a LICENSE file (MIT or Apache-2.0 recommended).

## 🗺️ Roadmap

- [x] Phase 1 MVP — single-file / batch conversion, MinerU engine integration
- [ ] Settings page (output format, concurrency, OCR toggle)
- [ ] OCR image text recognition (model structure reserved in `src/models/ocr.rs`)
- [ ] Drag-and-drop folder recursive conversion
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
