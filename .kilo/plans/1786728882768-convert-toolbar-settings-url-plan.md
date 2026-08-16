# 计划：ConvertPage 工具栏 + 设置页扩展 + URL 粘贴导入

## 1. ConvertPage 工具栏按钮打通

### 1.1 复制 Markdown
- **文件**: `frontend/src/pages/ConvertPage.tsx`
- **改动**: 第 143-147 行，把 `if (navigator.clipboard)` 改为实际调用 `navigator.clipboard.writeText(markdown).catch(...)`，加一个临时 success/error 反馈（简单 toast 或用 state 闪一下提示即可）
- **纯前端，无 Rust 改动**

### 1.2 保存 .md（Save As）
- **Rust 端** (`src/lib.rs`): 新增 `write_text_file(path: String, content: String) -> Result<(), String>` 命令，调用 `std::fs::write`
- **前端 API** (`frontend/src/api/tauriApi.ts`): 新增 `writeTextFile(path, content)` 函数
- **ConvertPage**: 点击"保存 .md"按钮时：
  1. 调用 `@tauri-apps/plugin-dialog` 的 `save()` 打开保存对话框，默认文件名根据 `currentTask.sourcePath` 推导（`xxx.md`），过滤器 `.md`
  2. 用户确认路径后调用 `writeTextFile(path, markdown)`
- 已有 `dialog:default` 权限，无需新增

### 1.3 打开文件夹
- **前端依赖**: `frontend/package.json` 新增 `"@tauri-apps/plugin-shell": "^2.0.0"`
- **权限**: `capabilities/default.json` 新增 `"shell:allow-open"`
- **ConvertPage**: 点击"打开文件夹"时：
  1. 从 `currentTask.outputPath` 提取父目录路径
  2. 调用 `@tauri-apps/plugin-shell` 的 `open(path)` 打开资源管理器
- **Rust 端**: `tauri-plugin-shell` 已在 `Cargo.toml` 中，无需改动

### 1.4 重新转换
- **ConvertPage**: 点击"重新转换"时：
  1. 从 `currentTask.sourcePath` 和 `outputPath` 提取源路径和输出目录
  2. 调用 `convertFile(sourcePath, outputDir)`（从 `tauriApi` import）
  3. 成功后更新 store：`finalizeTask` + `setCurrentTask`（与 HomePage 中单文件转换逻辑相同）
  4. 失败时显示错误
- **纯前端，无 Rust 改动**

### 1.5 按钮反馈
- 所有按钮点击后应有基本的 loading/disabled 状态或结果提示（如复制成功闪一下文字）
- 用 `useState` 控制按钮文字临时变化即可，无需额外 UI 库

---

## 2. 设置页扩展（转换配置）

### 2.1 新增设置 store
- **新建** `frontend/src/store/useSettingsStore.ts`
  ```ts
  interface SettingsStore {
    ocrEnabled: boolean;
    setOcrEnabled: (v: boolean) => void;
  }
  ```
  - 使用 zustand + localStorage 持久化（key: `omnimd_settings`）
  - `ocrEnabled` 默认 `false`
- 并发数不单独存 settings store，直接复用 `useTaskStore.concurrency`

### 2.2 SettingsPage 新增"转换"卡片
- **文件**: `frontend/src/pages/SettingsPage.tsx`
- 在 Appearance 卡片之后、About 卡片之前，新增一个 Card：
  - **标题**: `t("settings.conversion")`（"转换"）
  - **行 1 — 并发数**:
    - 标签: `t("settings.concurrency")`（"并发数"）
    - 输入框: `<Input type="number" min={1} max={16} value={concurrency} onChange={...} />`
    - `concurrency` 从 `useTaskStore` 解构，`onChange` 调 `setConcurrency`
  - **行 2 — OCR 文字识别**:
    - 标签: `t("settings.ocr")`（"OCR 文字识别"）
    - 开关: 一个 disabled 的 toggle/switch（或 checkbox），显示 `t("common.comingSoon")`
    - 值从 `useSettingsStore` 的 `ocrEnabled` 读取
  - **行 3 — 输出格式**（placeholder）:
    - 标签: `t("settings.outputFormat")`（"输出格式"）
    - 显示 "Markdown (默认)"，disabled 下拉框或纯文字
    - 意义：未来支持其他输出格式时扩展

### 2.3 i18n 新增键
- `zh-CN.ts` 和 `en.ts` 的 `settings` 段新增：
  ```ts
  conversion: "转换" / "Conversion",
  concurrency: "并发数" / "Concurrency",
  ocr: "OCR 文字识别" / "OCR Recognition",
  outputFormat: "输出格式" / "Output Format",
  ```
- 新增 `common` 段（或放 settings 内）：
  ```ts
  common: { comingSoon: "即将推出" / "Coming Soon" }
  ```

---

## 3. URL 粘贴导入

### 3.1 Rust 端 — 下载 URL 到临时文件
- **依赖**: `Cargo.toml` 新增 `reqwest = { version = "0.12", features = ["json"] }`
- **新命令** `src/lib.rs`:
  ```rust
  #[tauri::command]
  async fn download_url(url: String) -> Result<String, String> {
      // 1. 用 reqwest GET 下载 bytes
      // 2. 从 URL 提取文件名（最后一个 / 之后的部分），或生成随机名
      // 3. 保存到 std::env::temp_dir()/omnimd_downloads/ 下
      // 4. 返回本地路径字符串
  }
  ```
  - 考虑超时：`reqwest::Client::builder().timeout(Duration::from_secs(30)).build()`
  - 错误处理：网络错误、无效 URL、写入失败分别返回可读错误信息

### 3.2 前端 API
- `frontend/src/api/tauriApi.ts` 新增 `downloadUrl(url: string): Promise<string>`

### 3.3 HomePage 启用 URL 输入
- **文件**: `frontend/src/pages/HomePage.tsx`
- 移除 URL 输入框的 `disabled` 属性
- 添加 `onKeyDown` 或表单提交处理：
  - 当用户按 Enter 或点击附属按钮时
  - 调用 `downloadUrl(url)` 获取本地路径
  - 然后调用 `handleFiles([localPath])` 复用现有转换流程
  - 添加 loading 状态（`downloading`），下载期间禁用输入
  - 错误处理：显示错误提示（可用简单 alert 或内联文字）
- update i18n key `home.pasteUrl` 去掉 "（Phase 2）" 后缀

### 3.4 权限
- 无额外能力要求，`core:default` 包含 `command` 调用权限

---

## 涉及文件清单

| 文件 | 改动类型 | 说明 |
|------|---------|------|
| `Cargo.toml` | 改 | 新增 `reqwest` 依赖 |
| `src/lib.rs` | 改 | 新增 `write_text_file`、`download_url` 命令 |
| `capabilities/default.json` | 改 | 新增 `"shell:allow-open"` |
| `frontend/package.json` | 改 | 新增 `@tauri-apps/plugin-shell` |
| `frontend/src/api/tauriApi.ts` | 改 | 新增 `writeTextFile`、`downloadUrl` 函数 |
| `frontend/src/store/useSettingsStore.ts` | **新建** | OCR 开关状态 + localStorage 持久化 |
| `frontend/src/pages/ConvertPage.tsx` | 改 | 4 个按钮 onClick 逻辑 |
| `frontend/src/pages/SettingsPage.tsx` | 改 | 新增"转换"卡片（并发、OCR、输出格式） |
| `frontend/src/pages/HomePage.tsx` | 改 | 启用 URL 输入框，添加下载+转换逻辑 |
| `frontend/src/i18n/locales/zh-CN.ts` | 改 | 新增 settings 键、common 段 |
| `frontend/src/i18n/locales/en.ts` | 改 | 新增 settings 键、common 段 |

---

## 验证

1. `cd frontend && pnpm build` 通过（tsc + vite）
2. 如果 Rust 改动涉及，`cargo build` 通过（首次需下载 reqwest 依赖）
3. ConvertPage 四个按钮分别点击验证：
   - 复制：粘贴到别处确认内容正确
   - 保存 .md：弹出保存对话框，保存后文件可读
   - 打开文件夹：系统资源管理器打开输出目录
   - 重新转换：重新执行转换并更新结果
4. 设置页：
   - 并发数输入框修改后，首页和批处理页的并发数同步变化
   - OCR 开关存在但禁用，显示"即将推出"
   - 输出格式显示"Markdown（默认）"
5. URL 粘贴：输入可下载的 URL（如 raw 文件链接），按 Enter，下载后自动开始转换