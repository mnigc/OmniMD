# HomePage + History Redesign Plan

## Goals

1. **Merge HomePage + BatchPage**: Files added to homepage are listed; user clicks "Start Conversion" to begin. Progress shown per-file on homepage. Completed session tasks auto-clear when new files are added.
2. **New HistoryPage**: All processed documents shown with output mode, timestamp, status, open-file/open-folder actions.
3. **Remove BatchPage**: Sidebar has Home / Convert / History / Settings.

## File Manifest

| # | Action | File | Change |
|---|--------|------|--------|
| 1 | Modify | `frontend/src/types/index.ts` | Add `outputMode` to `ConversionTask`; add `HistoryEntry` type |
| 2 | Modify | `frontend/src/store/useTaskStore.ts` | Add `sessionTasks` + `history` (persisted); remove `tasks`; add `startConversion`, `addToHistory`, `loadHistory`, `clearSession`, `clearHistoryItem` |
| 3 | Modify | `frontend/src/App.tsx` | Add `history` page type; update sidebar; remove `batch` |
| 4 | Rewrite | `frontend/src/pages/HomePage.tsx` | Remove "Recent Conversions" card; add file list area + "Start Conversion" button + status bar |
| 5 | Delete | `frontend/src/pages/BatchPage.tsx` | Replaced by HomePage |
| 6 | Create | `frontend/src/pages/HistoryPage.tsx` | Full history table with open-file, open-folder, delete actions |
| 7 | Modify | `frontend/src/components/TaskItem.tsx` | Support `outputMode` display; add `onOpenFile`/`onOpenFolder` callbacks |
| 8 | Modify | `frontend/src/i18n/locales/zh-CN.ts` | Add `history` namespace; update `home` and `nav` entries |
| 9 | Modify | `frontend/src/i18n/locales/en.ts` | Same as zh-CN |
| 10 | Modify | `frontend/src/api/tauriApi.ts` | Add `readTextFile` API wrapper |
| 11 | Modify | `frontend/src/api/dialogs.ts` | (No change needed) |
| 12 | Modify | `src/lib.rs` | Add `read_text_file` Tauri command |
| 13 | Modify | `src/lib.rs` | Add `list_files_in_folder` Tauri command (for folder drop → individual file tasks) |

## Detailed Design

### 1. Types (`frontend/src/types/index.ts`)

```typescript
// Add outputMode to ConversionTask
export interface ConversionTask {
  id: string;
  sourcePath: string;
  outputPath: string;
  outputMode: OutputMode;       // NEW
  status: TaskStatus;
  progress: number;
  stage: ConversionStage;
  error: string | null;
  createdAt: number;
  completedAt: number | null;
}

// New type for history entries
export interface HistoryEntry {
  id: string;
  sourcePath: string;
  outputPath: string;
  outputMode: OutputMode;
  status: TaskStatus;
  error: string | null;
  createdAt: number;
  completedAt: number | null;
}
```

### 2. Store (`frontend/src/store/useTaskStore.ts`)

**State changes:**
- Remove `tasks: ConversionTask[]` (the old unified list)
- Add `sessionTasks: ConversionTask[]` (current batch, ephemeral)
- Add `sessionConverting: boolean` (is conversion in progress)
- Add `history: HistoryEntry[]` (persisted to localStorage)
- Add `converting: boolean` (to track conversion in progress)

**Actions:**
- `addToSession(paths, outputDir, outputMode)` → adds files as Pending tasks
- `removeFromSession(taskId)` → removes a single task
- `clearSession()` → clears all session tasks
- `startConversion()` → iterates session Pending tasks, calls `convertFile` individually with concurrency control, listens to Tauri events
- `addToHistory(entry)` → appends to history and persists
- `loadHistory()` → reads from localStorage
- `clearHistoryItem(id)` → removes one history entry
- `clearAllHistory()` → clears all history
- `updateSessionTaskProgress(taskId, progress, stage)` → live progress update
- `updateSessionTaskStatus(taskId, status, error?)` → status update

**Persistence:**
- `localStorage("omnimd_history")` stores `HistoryEntry[]`
- On app init, `loadHistory()` is called

**Conversion flow (startConversion):**
```
for each Pending task in sessionTasks:
  mark as Processing
  call convertFile(sourcePath, outputDir, outputMode, aiReadyOpts)
  on success: mark as Completed, add to history
  on failure: mark as Failed, add to history (with error)
concurrency = store's concurrency setting
```

The concurrency is managed as a simple loop with a semaphore pattern (N in-flight at a time using a counter + queue).

### 3. Backend (`src/lib.rs`)

Add two new commands:

```rust
#[tauri::command]
fn read_text_file(path: String) -> Result<String, String> {
    fs::read_to_string(&path)
        .map_err(|e| format!("读取文件失败: {}", e))
}

#[tauri::command]
fn list_files_in_folder(
    path: String,
) -> Result<Vec<String>, String> {
    // Returns list of supported files in the folder (non-recursive)
    // Uses file_utils::list_files_flat
}
```

### 4. App.tsx

```typescript
type Page = "home" | "convert" | "history" | "settings";
// Remove "batch"
```

Sidebar navigation:
- Home (Home icon) → `setPage("home")`
- Convert (ArrowRightLeft icon) → `setPage("convert")`  
- History (Clock icon) → `setPage("history")`  ← NEW
- Settings (Settings icon) → `setPage("settings")`

### 5. HomePage.tsx (Rewritten Structure)

```
┌───────────────────────────────────────────────────────────────┐
│  Header: title + selling points                                │
│                                                               │
│  [Output dir input] [Browse]  [Concurrency: 4]               │
│  [Standard] [AI Ready] [Obsidian]     ← OutputModeSelector    │
│                                                               │
│  DropZone (drag files/folders, click to pick)                 │
│  [Add Files] [📎 Paste URL]                                    │
│                                                               │
│  ─────────────────────────────────────────────               │
│  Current Session (N files) — hint: click Start to convert     │
│  ─────────────────────────────────────────────               │
│  ┌ TaskItem (Pending)  ┐  ┌ TaskItem (Processing)  ┐        │
│  │ file1.pdf   DOCX    │  │ file2.docx  DOCX       │        │
│  │ ⏳ Pending          │  │ ████████░ 80% ⏳ Processing│    │
│  └─────────────────────┘  └────────────────────────┘        │
│                                                               │
│  [▶ Start Conversion]  [🗑 Clear List]                       │
│                                                               │
│  Status bar: Processing:2  Completed:0  Failed:0  Pending:1  │
│  💡 Hint: "Check History for past conversions"                │
└───────────────────────────────────────────────────────────────┘
```

**Key behaviors:**
- **Adding files**: `DropZone.onFiles` calls `addToSession` (NOT auto-convert)
- **URL input**: Download then `addToSession`
- **Folder drop**: Call `listFilesInFolder` API, then `addToSession` for each file
- **Start button**: Disabled when no Pending tasks or when converting
- **During conversion**: Start button hidden, cancel button shown
- **After completion**: Tasks show Completed/Failed status; hint text shown
- **New file add after completion**: `clearSession()` then `addToSession(newFiles)`
- **While converting**: Adding files is allowed; new files appear as Pending

### 6. HistoryPage.tsx (New)

```
┌──────────────────────────────────────────────────────────────────┐
│  PageHeader: "Conversion History"  [Clear All]                   │
├───────┬──────────┬──────────┬──────────┬──────────┬─────────────┤
│ #     │ Filename │ Output   │ Time     │ Status   │ Actions     │
│       │          │ Mode     │          │          │             │
├───────┼──────────┼──────────┼──────────┼──────────┼─────────────┤
│ 1     │ report   │ AI Ready │ 08-15    │ ✅ Done  │ [📄 Open]  │
│       │ .pdf     │          │ 14:23    │          │ [📂 Folder] │
│       │          │          │          │          │ [🗑 Delete] │
├───────┼──────────┼──────────┼──────────┼──────────┼─────────────┤
│ 2     │ notes    │ Standard │ 08-14    │ ❌ Failed│ [📄 Open]  │
│       │ .docx    │          │ 10:00    │          │ [📂 Folder] │
│       │          │          │          │          │ [🗑 Delete] │
└───────┴──────────┴──────────┴──────────┴──────────┴─────────────┘
```

**Actions:**
- **Open File** (`📄`): Reads markdown from `outputPath` via `readTextFile`, navigates to ConvertPage with the content
- **Open Folder** (`📂`): Calls `shell.open(outputPath.parent)` to open in file explorer
- **Delete** (`🗑`): Removes entry from history store

### 7. TaskItem.tsx Modifications

- Show `outputMode` badge (small label: Standard / AI Ready / Obsidian)
- Accept `onOpenFile` and `onOpenFolder` callbacks (for history mode)
- Accept `showActions` prop to control which action buttons appear

### 8. i18n Additions

**`zh-CN.ts`** — add `history` namespace:

```typescript
nav: {
  // ... keep existing, change "batch" to "history"
  history: "转换历史",
},
history: {
  title: "转换历史",
  empty: "暂无转换记录",
  emptyHint: "完成文档转换后，记录将显示在此处",
  fileName: "文件名",
  outputMode: "输出模式",
  time: "转换时间",
  status: "状态",
  actions: "操作",
  openFile: "打开文件",
  openFolder: "打开文件夹",
  delete: "删除记录",
  clearAll: "清空历史",
  clearConfirm: "确定清空所有历史记录？",
},
home: {
  // ... keep existing, add:
  startConversion: "开始转换",
  noFilesInSession: "暂无文件，请先添加文件",
  clearSession: "清空列表",
  viewHistoryHint: "转换完成后可前往「转换历史」查看所有记录",
  conversionComplete: "转换完成，共 {count} 个文件",
  converting: "转换中...",
  addingFiles: "添加文件到本次转换",
  sessionTitle: "待转换文件",
  newSessionHint: "添加新文件将清空当前已完成的任务",
},
```

### 9. Execution Order

1. **Backend**: Add `read_text_file` and `list_files_in_folder` commands to `src/lib.rs`
2. **Types**: Update `types/index.ts`
3. **Store**: Rewrite `useTaskStore.ts` with session + history
4. **API**: Add `readTextFile` + `listFilesInFolder` to `tauriApi.ts`
5. **TaskItem**: Add outputMode display, openFile/openFolder callbacks
6. **HomePage**: Rewrite with merged batch functionality
7. **HistoryPage**: Create new page
8. **App.tsx**: Update routing + sidebar
9. **Delete**: `BatchPage.tsx`
10. **i18n**: Update both locale files

## Validation

1. `npm run build` compiles without errors
2. `cargo build` compiles without errors
3. Add files → they appear in the session list, NOT auto-converted
4. Click "Start Conversion" → progress shown per-file
5. After completion → hint to check history appears
6. Add new files → old completed session tasks clear, new ones appear
7. History page shows all completed tasks with correct output mode, time, status
8. Open file from history → reads markdown, shows in ConvertPage
9. Open folder from history → opens file explorer
10. Delete from history → entry removed