<div align="center">

[English](README.md) | [简体中文](README.zh.md) | [한국어](README.ko.md) | [日本語](README.ja.md)

---

# OmniMD

### Anything to Markdown

PDF、Word、Excel、PowerPoint、EPUB、HTMLなど、さまざまなドキュメントをクリーンなMarkdownに変換するクロスプラットフォーム・デスクトップアプリケーションです。

**Tauri 2** + **Rust** + **React** で構築され、コア変換エンジンは [MinerU](https://github.com/opendatalab/MinerU) 3.x が提供します。

</div>

---

## ✨ 主な機能

- **20+ フォーマット対応** — PDF、DOC/DOCX、PPT/PPTX、XLS/XLSX、EPUB、CSV、TXT、HTML、ODT/ODS/ODP、RTF など
- **バッチ変換** — 複数ファイルを一度にドラッグし、並行変換とリアルタイムの進捗フィードバック
- **アセット抽出** — ドキュメント内の画像などのリソースを自動的に `assets/` ディレクトリに保存
- **スマートフォーマット検出** — マジックバイトによるフォーマット自動検出、拡張子に依存しない
- **テキストパススルー** — プレーンテキスト、JSON、XML、HTML を直接 Markdown ブロックとして変換
- **ローカル優先** — すべての変換はローカルで完結、ファイルのアップロードなし、プライバシー保護
- **ネイティブデスクトップ体験** — Tauri でパッケージングされた軽量・高速なネイティブアプリ

## 📸 スクリーンショット

> TODO: スクリーンショット追加

アプリは3つのメインページで構成されます：

| ページ | 用途 |
|--------|------|
| **Home** | ファイルブラウジングとクイックアクセス |
| **Convert** | 単一ファイル変換、出力プレビュー |
| **Batch** | バッチ変換、並行制御と進捗トラッキング |

## 🚀 クイックスタート

### 前提条件

| 依存関係 | バージョン | 備考 |
|----------|-----------|------|
| [Node.js](https://nodejs.org) | ≥ 18 (LTS) | フロントエンド・ランタイム |
| [pnpm](https://pnpm.io) | ≥ 8 | パッケージマネージャー |
| [Rust](https://www.rust-lang.org/tools/install) | stable | バックエンド・コンパイラ |
| MSVC Build Tools | — | Windows 必須（「C++ によるデスクトップ開発」を選択） |

### インストールと実行

```bash
# 1. リポジトリをクローン
git clone https://github.com/<your-org>/OmniMD.git
cd OmniMD

# 2. フロントエンド依存関係をインストール
cd frontend
pnpm install

# 3. Tauri CLI を追加（初回のみ）
pnpm add -D @tauri-apps/cli

# 4. 開発モードを起動（Rust バックエンド コンパイル + フロントエンド ホットリロード）
pnpm tauri dev
```

初回起動時は多数の Rust 依存関係をコンパイルするため 5〜15 分かかります。以降のインクリメンタルビルドは高速です。準備が完了するとデスクトップウィンドウが自動的に表示されます。

## 📦 リリースビルド

```bash
cd frontend
pnpm tauri build
```

成果物は `frontend/src-tauri/target/release/bundle/` に配置されます（Windows では `.msi` / `.exe` インストーラー）。

## 🏗️ プロジェクト構成

```
OmniMD/
├── src/                    # Rust バックエンド
│   ├── main.rs             # エントリポイント
│   ├── lib.rs              # Tauri コマンド登録（フロントエンド API）
│   ├── pipeline.rs         # 変換パイプライン（読込 → 変換 → 書出）
│   ├── file_utils.rs       # パスヘルパー / フォーマットホワイトリスト
│   ├── engine/           # MinerU エンジン統合 (HTTP client + ランタイム管理)
│   └── models/             # Document / Task / Asset データ構造
├── frontend/               # React + TypeScript フロントエンド
│   ├── src/
│   │   ├── App.tsx         # アプリのシェルとナビゲーション
│   │   ├── pages/          # Home / Convert / Batch ページ
│   │   ├── api/            # Rust バックエンド invoke ラッパー
│   │   ├── store/          # zustand 状態管理
│   │   ├── components/     # 再利用コンポーネント
│   │   └── types/          # 共有型定義
│   └── vite.config.ts      # Vite 設定（ポート 1420）
├── tauri.conf.json         # Tauri アプリ設定
├── capabilities/           # Tauri 権限設定
├── Cargo.toml              # Rust 依存関係
└── tests/                  # 結合テスト + fixtures
```

## 🔌 フロントエンド–バックエンド通信

フロントエンドは Tauri の `invoke` を介して登録された Rust コマンドを呼び出します。[`src/lib.rs`](src/lib.rs) を参照：

| コマンド | パラメータ | 戻り値 | 説明 |
|----------|-----------|--------|------|
| `convert_file` | `sourcePath`, `outputDir` | `ConversionResult` | 単一ファイル変換 |
| `get_supported_formats` | — | `string[]` | 対応拡張子リスト |
| `get_converter_info` | — | `ConverterInfo` | 変換器名と対応フォーマット |

変換中の進捗は Tauri イベント `task-progress` / `task-status` でプッシュされ、フロントエンドはこれをリッスンしてリアルタイム UI を更新します。

## 🧪 テスト

```bash
# Rust ユニット + 結合テスト
cargo test

# フロントエンド型チェック + ビルド
cd frontend && pnpm build
```

テスト fixtures は [`tests/fixtures/`](tests/) にあります。

## 🛠️ 技術スタック

| レイヤー | 技術 |
|----------|------|
| デスクトップフレームワーク | Tauri 2 |
| バックエンド言語 | Rust 2021 edition |
| 変換エンジン | [MinerU](https://github.com/opendatalab/MinerU) 3.x (via `mineru-api`) |
| 非同期ランタイム | tokio |
| フロントエンドフレームワーク | React 18 + TypeScript 5 |
| ビルドツール | Vite 5 |
| スタイリング | Tailwind CSS 3 |
| 状態管理 | zustand |
| Markdown レンダリング | react-markdown + remark-gfm |

## 📄 ライセンス

TODO: LICENSE ファイルを追加（MIT または Apache-2.0 を推奨）。

## 🗺️ ロードマップ

- [x] Phase 1 MVP — 単一ファイル / バッチ変換、MinerU エンジン統合
- [ ] Settings ページ（出力フォーマット、並行数、OCR トグル）
- [ ] OCR 画像テキスト認識（モデル構造は `src/models/ocr.rs` に予約済み）
- [ ] ドラッグ＆ドロップのフォルダ再帰変換
- [ ] クロスプラットフォームビルド（macOS / Linux）

## 📖 追加ドキュメント

詳細な開発、デバッグ、パッケージング手順は **[開発・デバッグガイド](docs/开发调试指南.md)**（中国語）を参照してください。

## 🤝 コントリビュート

Issue と PR を歓迎します。PR 提出前に以下を確認してください：

```bash
cargo test                    # バックエンドテスト合格
cd frontend && pnpm build     # フロントエンドビルド エラーなし
```

---

<div align="center">

Built with Tauri · Rust · React

</div>
