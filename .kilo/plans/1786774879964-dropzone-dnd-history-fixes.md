# Fix: DropZone 拖拽失效 / 文件夹内容不可见 / 历史打开文件无反馈

## Context

After the HomePage + History redesign, users reported three issues on Windows/WebView2:

1. **拖入无效 + 区域过大** — `DropZone` 依赖 `dataTransfer.files[i].path`（Tauri v1 时代给 WebKit 注入的属性，Windows WebView2 上不存在），导致拖文件时拿不到路径、静默回退到打开"选文件"对话框；且拖拽框（`min-h-[160px]`）是个大空框，与下方独立"添加文件"按钮功能重复。
2. **"批处理队列"按钮即文件夹选择，且选中文件夹后看不到其下文件** — 按钮文案沿用已删除 BatchPage 的 `t("batch.title")`（"批处理队列 →"），误导；选文件夹后 `handleFolder` 走 `listFilesInFolder`，但任何失败被 `catch {}` 静默吞掉（典型：后端二进制未重建、`list_files_in_folder` 命令不存在），用户看不到结果。
3. **历史页"打开文件/打开文件夹"无反应** — `HistoryPage` 两个 handler 全部 `catch {}` 静默吞错；`read_text_file`/`list_files_in_folder` 是上一轮新增的后端命令，旧二进制里不存在时 `invoke` 直接 reject，前端无任何提示。

## Goal

- 拖拽真实可用（全窗口生效、Windows 也拿得到绝对路径），拖拽区改紧凑、不再像大按钮。
- 文件夹输入（按钮/拖拽）后，其下支持的文件显式进入"待转换文件"列表。
- 历史页打开文件/文件夹失败时给出可见错误提示，不静默。

## File Manifest

| # | Action | File | Change |
|---|--------|------|--------|
| 1 | Rewrite | `frontend/src/components/DropZone.tsx` | 用 Tauri `onDragDropEvent` 取真实路径；紧凑横栏（`px-4 py-3`、icon 20）；内置"选择文件/选择文件夹"按钮；移除 `disabled` prop 与旧 `f.path` 逻辑（保留 DOM 回退） |
| 2 | Modify | `frontend/src/pages/HomePage.tsx` | 新增 `addInputPaths(paths)` 统一入口（对每个路径调 `listFilesInFolder` 展开文件夹、去重后 `handleFiles`）；`handleFolder`/`handleAddFiles`/URL 都改走它；移除 DropZone 的 `disabled`/`min-h-[160px]`；删掉下方独立"添加文件"按钮（URL 行保留）；清理未用 import（`pickFiles`、`FilePlus`） |
| 3 | Modify | `frontend/src/pages/HistoryPage.tsx` | 打开文件/文件夹不再静默吞错：失败时设置 `actionError` 状态，页面顶部显示 destructive 提示，4s 后自动清除；空目录也提示 |
| 4 | Modify | `frontend/src/i18n/locales/zh-CN.ts` | 加 `home.chooseFolder`、`history.openFileError`、`history.openFolderError` |
| 5 | Modify | `frontend/src/i18n/locales/en.ts` | 同上英译 |

后端与 capabilities 无需改动：`list_files_in_folder` 对单个文件原样返回、对文件夹扁平列支持格式，可同时当作文件/文件夹展开器；`onDragDropEvent` 走 `core:event`（`core:default` 已含）；`shell:allow-open` 已配置。

## Detailed Changes

### 1. DropZone.tsx

Props：`{ onFiles: (paths: string[]) => void; onFolder?: (path: string) => void; formats: string[]; className?: string }`（删除 `disabled`）。

`useEffect`（仅挂载一次，deps `[onFiles]`）：
```ts
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import type { UnlistenFn } from "@tauri-apps/api/event";

useEffect(() => {
  let unlisten: UnlistenFn | undefined;
  (async () => {
    try {
      const appWindow = getCurrentWebviewWindow();
      unlisten = await appWindow.onDragDropEvent((event) => {
        const type = event.payload.type;
        if (type === "enter") {
          const paths = (event.payload as { paths?: string[] }).paths ?? [];
          const single = paths.length === 1 ? paths[0] : undefined;
          setIsFolderDrag(single !== undefined && !(single.split(/[\\/]/).pop() ?? "").includes("."));
          setIsDragging(true);
        } else if (type === "over") setIsDragging(true);
        else if (type === "leave") { setIsDragging(false); setIsFolderDrag(false); }
        else if (type === "drop") {
          setIsDragging(false); setIsFolderDrag(false);
          const paths = (event.payload as { paths?: string[] }).paths ?? [];
          if (paths.length) { tauriDndReady.current = true; onFiles(paths); }
        }
      });
    } catch { /* 非 Tauri，走 DOM 回退 */ }
  })();
  return () => unlisten?.();
}, [onFiles]);
```

行为要点：
- 拖入/拖出/悬停高亮由 Tauri 事件驱动；`drop` 把真实路径直接交给 `onFiles`（文件与文件夹混合原样传出，展开逻辑在 HomePage）。
- DOM 回退：仅 `preventDefault`；当 `tauriDndReady.current === true` 时 drop 直接 return（避免与 Tauri 事件重复）；否则尝试旧 `f.path`。
- 渲染：单行紧凑横栏，`flex items-center`，icon 区 `p-2.5 rounded-full`（size 20），文字区 `flex-1 min-w-0 truncate`（沿用 `t("dropzone.*")` 文案 + `selectedCount` 提示），右侧两个 `Button size="sm" variant="outline"`（`e.stopPropagation()`）：选择文件（`t("home.addFiles")`）、选择文件夹（`t("home.chooseFolder")`）。

### 2. HomePage.tsx

```ts
const addInputPaths = useCallback(async (paths: string[]) => {
  if (!paths.length) return;
  let expanded: string[] = [];
  for (const p of paths) {
    try { expanded.push(...(await listFilesInFolder(p))); }
    catch { expanded.push(p); } // 失败按单文件处理，错误在任务项内展示
  }
  const unique = [...new Set(expanded)];
  if (unique.length === 0) return;
  handleFiles(unique);
}, [handleFiles]);
```

- `handleFiles`（现有会话清空/追加逻辑）保持同步不变，仅由 `addInputPaths` 调用。
- `handleFolder = (dir) => addInputPaths([dir])`；`handleAddFiles`（若保留按钮也可改走它，但 DropZone 已内建选择按钮，一并删除下方独立按钮）；`handleUrlSubmit` 改 `addInputPaths([localPath])`。
- DropZone 用法：`<DropZone onFiles={addInputPaths} onFolder={handleFolder} formats={supportedFormats} />`（去掉 `disabled` 与 `min-h-[160px]`）。
- 移除：独立"添加文件"按钮行、未用的 `FilePlus`/`pickFiles` import；保留 URL 行与 `FolderOpen`（输出目录浏览用）。

### 3. HistoryPage.tsx

```ts
const [actionError, setActionError] = useState("");
useEffect(() => {
  if (!actionError) return;
  const timer = setTimeout(() => setActionError(""), 4000);
  return () => clearTimeout(timer);
}, [actionError]);
```
- `handleOpenFile` catch → `setActionError(t("history.openFileError") + ": " + (err?.message ?? err))`。
- `handleOpenFolder`：目录为空或 `open(dir)` 失败 → `setActionError(t("history.openFolderError"))`（成功时清除错误）。
- 页面顶部（`PageHeader` actions 下方/上方）渲染 `actionError && <div className="...text-destructive...">`。
- 打开文件成功逻辑不变：`readTextFile(entry.outputPath)` → 构造 task/result → `setCurrentTask` → `onNavigate?.("convert")`。

### 4. i18n

zh-CN：`home.chooseFolder: "选择文件夹"`；`history.openFileError: "打开文件失败"`；`history.openFolderError: "打开文件夹失败"`。
en：`home.chooseFolder: "Choose Folder"`；`history.openFileError: "Failed to open file"`；`history.openFolderError: "Failed to open folder"`。

## Execution Order

1. `DropZone.tsx` 重写（先定型组件接口）
2. `HomePage.tsx`：`addInputPaths` + 接入 + 清理 import/按钮
3. `HistoryPage.tsx`：错误展示
4. 两个 locale 文件补 3 个 i18n key

## Validation

- `npm run build`（tsc + vite）通过，无未用 import。
- `cargo build` 通过（后端无改动，跑一次确认环境）。
- 用 `npm run tauri dev` **整体重建后**手动验证：
  1. 拖 1 个/多个文件到窗口任意位置 → 进入"待转换文件"列表，同一文件不重复。
  2. 拖入文件夹 → 其下支持格式文件逐条进入列表；"选择文件夹"按钮同样展开。
  3. 历史页"打开文件" → 跳转 Convert 页显示 markdown；"打开文件夹" → 打开资源管理器。
  4. 在旧二进制/文件缺失场景下，历史操作与文件夹列表失败时能看到明确错误提示。

## Open Questions / Risks

- 若用户当前运行的是旧 Rust 二进制，`read_text_file`/`list_files_in_folder` 不存在，问题 2/3 依旧失败，但本次会把错误显式暴露给用户；需提醒完整重建（`npm run tauri dev`）后生效。
- `onDragDropEvent` 在纯浏览器 `vite dev`（非 Tauri）下不可用，已保留 DOM 回退兜底，不影响 Tauri 实际运行。