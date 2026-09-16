/**
 * Browser-only dev preview shim.
 *
 * The Tauri runtime injects `window.__TAURI_INTERNALS__` into the webview.
 * When the frontend is served by plain Vite (no Rust backend), that object is
 * missing and every Tauri API throws synchronously — which crashes the whole
 * tree through the error boundary and makes UI-only iteration impossible.
 *
 * Installing this minimal stub keeps the shell renderable: backend calls
 * simply reject, so pages fall back to their empty states. A couple of cheap,
 * static commands are answered with canned data so home-page chrome (format
 * chips, version label) still renders realistically. Dev builds only — the
 * guard is a no-op in production and inside the real Tauri webview.
 */
export function installBrowserPreviewStub(): void {
  if (!import.meta.env.DEV) return;
  if ("__TAURI_INTERNALS__" in window) return;

  const reject = () =>
    Promise.reject(new Error("Tauri backend unavailable (browser preview)"));

  const canned: Record<string, unknown> = {
    get_supported_formats: [
      "pdf", "doc", "docx", "docm", "ppt", "pps", "pot", "pptx", "pptm",
      "ppsx", "ppsm", "xls", "xlsx", "xlsm", "xlsb", "odt", "ods", "odp",
      "rtf", "epub", "csv",
    ],
    get_app_version: "v0.1.0",
  };

  // ---- Knowledge-base preview data: a deep, long-named folder tree so the
  // library UI can be exercised without the Rust backend. ----
  const ws = {
    id: 1,
    name: "测试数据",
    path: "D:/测试数据",
    createdAt: "2026-01-01T00:00:00Z",
    lastOpenedAt: null,
  };
  const folder = (name: string, parent: string, docCount: number) => ({
    name,
    path: `${parent}/${name}`,
    docCount,
  });
  const R = ws.path;
  const subfolders: Record<string, ReturnType<typeof folder>[]> = {
    "": [folder("content-main", R, 12), folder("glossary-long-name", R, 3)],
    [`${R}/content-main`]: [folder("en-us", `${R}/content-main`, 12)],
    [`${R}/content-main/en-us`]: [
      folder("games", `${R}/content-main/en-us`, 6),
      folder("publishing_games", `${R}/content-main/en-us`, 4),
      folder("learn_web_development", `${R}/content-main/en-us`, 8),
    ],
    [`${R}/content-main/en-us/games`]: [
      folder("anatomy", `${R}/content-main/en-us/games`, 2),
      folder("game_distribution_and_monetization", `${R}/content-main/en-us/games`, 2),
    ],
    [`${R}/content-main/en-us/games/game_distribution_and_monetization`]: [
      folder("promotion_strategies", `${R}/content-main/en-us/games/game_distribution_and_monetization`, 2),
    ],
  };
  const docsFor = (f: string) =>
    Array.from({ length: 40 }, (_, i) => ({
      id: f.length * 100 + i,
      workspaceId: 1,
      path: `${f === "" ? "root" : f.split("/").pop()}/doc-${i + 1}.md`,
      title: `Document ${String(i + 1).padStart(2, "0")} — sample entry`,
      fileSize: 1024 * (i + 1),
      favorite: i === 0,
      source: null,
      createdAt: "2026-01-01T00:00:00Z",
      openedAt: "2026-09-01T00:00:00Z",
    }));

  const library: Record<string, (args: Record<string, unknown>) => unknown> = {
    list_workspaces: () => [ws],
    get_active_workspace: () => ws,
    scan_workspace: () => ({ indexed: 24, updated: 0, removed: 0, total: 24 }),
    list_subfolders: (args) => subfolders[String(args.folder ?? "")] ?? [],
    list_documents: (args) => docsFor(String(args.folder ?? "")),
    list_favorites: () => docsFor("").slice(0, 1),
    list_recent: () => docsFor(""),
    search_documents: () => [],
    read_text_file: () =>
      Promise.resolve(
        [
          "# 示例文档",
          "",
          "这是一段用于**浏览器预览**的 Markdown 正文，包含*斜体*、~~删除线~~、[链接](https://example.com) 和 `inline code`。",
          "",
          "## 二级标题",
          "",
          "- 列表项一",
          "- 列表项二",
"  - 嵌套列表项",
          "",
          "> 引用块：好的排版让写作更专注。",
          "",
          "```rust",
          "fn main() {",
          "    println!(\"hello\");",
          "}",
          "```",
          "",
          "---",
          "",
          "### 三级标题",
          "",
          "| 列 A | 列 B |",
          "| ---- | ---- |",
          "| 1    | 2    |",
        ].join("\n"),
      ),
  };

  (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__ = {
    metadata: {
      currentWindow: { label: "main" },
      currentWebview: { label: "main" },
    },
    invoke: (cmd: string, args: Record<string, unknown> = {}) =>
      cmd in canned
        ? Promise.resolve(canned[cmd])
        : library[cmd]
          ? Promise.resolve(library[cmd](args))
          : reject(),
    plugins: {},
    resources: {},
  };
}
