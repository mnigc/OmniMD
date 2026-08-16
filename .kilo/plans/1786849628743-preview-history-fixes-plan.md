# Preview & History Fixes Plan

## Scope

Fix 4 user-reported issues around the preview (ConvertPage) and history (HistoryPage) workflows.

---

## Issue 1 — 转换历史的"打开文件夹"失败

**Root cause**: `HistoryPage.handleOpenFolder` computes `dirname(entry.outputPath)` which may return a relative path; the plugin-shell `open()` call then fails because it cannot resolve it relative to the WebView working directory. ConvertPage's `打开文件夹` button has a similar problem (string-manipulation path split, not resolved to absolute).

**Fix**:

1. `HistoryPage.tsx` — rewrite `handleOpenFolder`:
   - Use `resolve(await dirname(entry.outputPath))` unconditionally (do not skip the `resolve` when the result looks absolute; Tauri's `resolve` is a no-op for absolute paths on that branch).
   - Wrap with `try/catch` and show the existing `actionError` toast on failure.
   - Alternative simpler path: drop the local `open` and call the backend `openFolder(dir)` via `tauriApi.openFolder()` (which uses `cmd /c start` on Windows). Prefer the backend helper since the same primitive already exists.

2. `ConvertPage.tsx` — same fix for the 打开文件夹 button:
   - Replace the manual `replace(/\\/g,"/").split("/").slice(0,-1).join("/")` with `resolve(dirname(outputPath))`.
   - Call `openFolder()` from `tauriApi` (import it).

**i18n**: already covered (`history.openFolderError`, `convert.openFolder`).

---

## Issue 2 — 预览页面默认空状态文案和按钮

**Change** (`zh-CN.ts`, `en.ts`):
- `convert.noFileHint`: `"在首页拖入文件开始转换"` → `"请从转换历史打开预览"`
- Add new key `convert.openHistory` = `"打开转换历史"`.

**Change** (`ConvertPage.tsx`):
- The empty-state button currently navigates to `"home"`. Change to `onNavigate("history")`.
- Button label: `{t("convert.openHistory")}`.
- Icon stays as `<ArrowLeft size={14} />` (acceptable).

---

## Issue 3 — 从转换历史打开预览后，去掉部分按钮 + 全局"复制成功"提示

### 3a. Detect "history mode" in the Convert page

Add a field to the store so ConvertPage knows whether the current preview was loaded from history vs. from a live conversion.

**`frontend/src/store/useTaskStore.ts`**:
- Add `previewSource: "live" | "history" | null` to the `TaskStore` interface and state (default `null`).
- `setCurrentTask` signature stays the same, but accept an optional third arg `source?: "live" | "history" | null` (default `"live"`); set it in state.
- `HistoryPage.handleOpenFile`, after `setCurrentTask(task, result)`, also call the store to set `previewSource` to `"history"`.
  - Simpler: extend `setCurrentTask(task, result, source)` and call `setCurrentTask(task, result, "history")` from HistoryPage.
  - From the reconvert path in ConvertPage and from HomePage/other places, call `setCurrentTask(task, result, "live")` (or no third arg — defaults to `"live"`).
- When the user clicks the empty-state button to go to history (issue 2), reset `previewSource` to `null`.

### 3b. Hide buttons in history mode

In `ConvertPage.tsx`, when `previewSource === "history"`, hide:
- 复制纯文本 button
- 保存 .md button
- 重新转换 button

Keep visible: 复制 Markdown, 打开文件夹, AI 优化 (already disabled placeholder).

### 3c. Global "复制成功" toast

Create a small shared `Toast` component (`frontend/src/components/Toast.tsx`):
- Fixed-position bottom-center pill, green accent, "复制成功" text.
- Auto-dismiss after 2 s.
- Renders at the App root level with a `toast` state (e.g. via a tiny context or by lifting state into App), so it shows even when the ConvertPage button fires.

Simpler approach (no context): add a `toast` piece of state in `App.tsx` and a `showToast(message, duration)` global accessible via a module-level function (`useToast.ts` — simple event-based pub/sub).

**Recommended**: create `src/lib/toast.ts` with `showToast(msg: string, durationMs: number)` and a portal that renders inside `App.tsx`. Event-based, no context needed.

In ConvertPage:
- Replace the two local `copied`/`copiedPlain` state + per-button `setCopied` logic with a single `showToast(t("toast.copied"), 2000)` call after successful `clipboard.writeText`.
- Remove local `copied` / `copiedPlain` state variables.
- Button label stays static: `{t("convert.copyMarkdown")}`.

**i18n additions** (`zh-CN.ts`, `en.ts`):
- `toast.copied` = `"复制成功"`
- `convert.openHistory` = `"打开转换历史"`

---

## Issue 4 — 分栏视图中间拖拽调整宽度

Implement a draggable divider between the Markdown editor and the preview panel in split mode.

**Approach** (React + pointer events, no new deps):

1. Replace the two `flex-1` panels with explicit `width` state:
   - `splitRatio: number` (e.g. 0.5 = 50/50), persisted to `localStorage` under key `omnimd_split_ratio`.
   - Each panel gets `style={{ flex: "0 0 auto", width: `${splitRatio * 100}%` }}`.

2. Insert a divider element between the panels:
   - `width: 6px`, `cursor: col-resize`, centered hover highlight.
   - On `pointerDown` → `pointerMove` on `document` → `pointerUp` on `document`, update `splitRatio`.
   - Clamp ratio to `[0.2, 0.8]`.
   - `useEffect` to add/remove global `pointermove`/`pointerup` listeners (only while dragging) to allow dragging outside the narrow divider.
   - `e.preventDefault()` on pointermove to avoid text selection during drag.

3. Reset to 0.5 if the persisted value is missing.

**Scope note**: Only the split-mode divider needs this. Edit-only and preview-only modes are single-panel and unaffected.

---

## Affected files

| File | Change |
|---|---|
| `frontend/src/pages/ConvertPage.tsx` | Empty-state copy + history nav; history-mode conditional rendering; `openFolder` via backend; toast call; split-ratio draggable divider |
| `frontend/src/pages/HistoryPage.tsx` | `handleOpenFolder` → use `openFolder` backend helper; `setCurrentTask(task, result, "history")` |
| `frontend/src/store/useTaskStore.ts` | Add `previewSource` field; `setCurrentTask` accepts optional `source` arg |
| `frontend/src/i18n/locales/zh-CN.ts` | New/updated keys |
| `frontend/src/i18n/locales/en.ts` | Mirror keys |
| `frontend/src/api/tauriApi.ts` | Already exports `openFolder` — reuse |
| `frontend/src/lib/toast.ts` | New — global event-based toast helper |
| `frontend/src/App.tsx` | Mount toast portal |
| `frontend/src/types/index.ts` | No change needed |

---

## Validation

1. **打开文件夹 (history)**: create a conversion, go to 转换历史, click the folder icon → Explorer opens the output directory.
2. **打开文件夹 (preview)**: after conversion, click 打开文件夹 on preview page → Explorer opens the output directory.
3. **Preview empty state**: load preview page with no task → shows "请从转换历史打开预览" + "打开转换历史" button → clicking navigates to history page.
4. **History-mode preview**: click 打开文件 on a history row → preview page shows without 复制纯文本 / 保存 / 重新转换 buttons; 复制 Markdown works and shows a "复制成功" toast that disappears after 2 s.
5. **Live-mode preview**: convert a file from home page → preview page shows all buttons (including 复制纯文本, 保存, 重新转换).
6. **Draggable split**: enter split mode, drag the divider → panels resize; refresh page → ratio restored from localStorage.

---

## Risks / Notes

- `resolve()` from `@tauri-apps/api/path` handles relative and absolute paths; safe to call unconditionally.
- Dragging with pointer events can conflict with textarea scroll; `e.preventDefault()` on `pointermove` during drag prevents text selection.
- No new npm dependencies needed.