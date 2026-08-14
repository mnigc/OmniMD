# OmniMD — Tauri Dev 卡死 "Waiting for frontend dev server" 修复计划

## 当前状态

- **前端 (Vite)**：`cd frontend && pnpm dev` → 正常启动，监听 `http://localhost:1420` ✅
- **Rust 工具链**：rustc 1.97.1 / cargo 1.97.1，已加入 **系统 PATH** ✅
- **MSVC Build Tools**：`C:\Program Files (x86)\Microsoft Visual Studio\18\BuildTools` ✅
- **项目结构**：已恢复至根目录结构（`Cargo.toml`、`src/`、`tauri.conf.json`, `icons/`, `capabilities/`、`gen/`、`tests/` 都在 `OmniMD/` 根下） ✅
- **根 `package.json`**：已创建，含 `@tauri-apps/cli` + `"dev": "cd frontend && pnpm dev"` + `"tauri": "tauri"` ✅
- **`tauri.conf.json`**：`frontendDist` 已改为 `"frontend/dist"` ✅
- **Tauri CLI**：`pnpm tauri dev` 从根目录运行，**但卡死** "Waiting for your frontend dev server to start on http://localhost:1420/..."

## 根原因

Tauri CLI 在 `tauri.conf.json` 中找不到 `build.beforeDevCommand`，尝试**自动检测**前端 dev 服务器的启动命令，但无法正确识别 root `package.json` 的 `"dev": "cd frontend && pnpm dev"`。

Tauri 2 的自动检测只在前端目录就近 `package.json` 时灵活；当 `tauri.conf.json` 与前端子目录分离时，**必须显式指定 `beforeDevCommand`**。

## 影响范围

| 文件 | 操作 |
|------|------|
| `tauri.conf.json` | **编辑**：新增 `build.beforeDevCommand` + `build.beforeBuildCommand` |
| `README.md` | 更新启动命令 |
| `README.zh.md` | 更新启动命令 |
| `docs/开发调试指南.md` | 更新启动命令 + FAQ |

## 修改细节

### 1. `tauri.conf.json` — 添加 beforeDevCommand

```json5
{
  "build": {
    "frontendDist": "frontend/dist",
    "devUrl": "http://localhost:1420",
    "beforeDevCommand": "pnpm dev",      // ← 新增，从根目录运行 npm 脚本
    "beforeBuildCommand": "pnpm build"   // ← 新增
  },
  "app": {
    // ... 原样不变
  },
  "bundle": {
    "icon": ["icons/icon.ico"],
    // ... 原样不变
  }
}
```

**原理**：`beforeDevCommand: "pnpm dev"` 让 Tauri CLI 执行 root `package.json` 中的 `"dev": "cd frontend && pnpm dev"`，即 `cd frontend && pnpm dev`，从而 Vite 被正确启动。

### 2. `README.md` — 更新安装/运行/打包指令

- **Get Started** 区段：在项目根 `pnpm install` 安装 `@tauri-apps/cli`；移除 "Add Tauri CLI" 单独步骤
- **Build for Release**：`cd frontend && pnpm tauri build` → `pnpm tauri build` (根目录)

### 3. `docs/开发调试指南.md` — 更新

- **第 1.4 节**：改为在根目录 `pnpm install`
- **第 2.1/2.2 节**：在根目录 `pnpm install` + `pnpm tauri dev`
- **第 6 节**：`pnpm tauri build` (根目录)
- **第 8 表**：更新所有 `pnpm tauri xxx` 的运行目录为"项目根目录"
- **新增 FAQ**：`pnpm tauri dev` 卡死 → 在项目根目录运行

### 4. `README.zh.md` — 同上翻译版

## 验证步骤

| 步骤 | 预期结果 |
|------|----------|
| `cd frontend && pnpm dev` | Vite 服务正常启动在 `:1420` |
| `rustc --version` (新 terminal) | 输出 1.97.1 |
| `pnpm tauri dev` (从 `OmniMD/` 根) | Tauri CLI 启动 Vite + 编译 Rust，弹出窗口 |

## 回退方案

若 `beforeDevCommand` 仍无法正常发起 Vite（偶尔 Windows 路径问题），可在 `tauri.conf.json` 换用绝对路径脚本：

```json5
"beforeDevCommand": { "cmd": "pnpm", "args": ["dev"], "cwd": "." }
```

## 风险

- `beforeDevCommand` 在 Windows 上偶有不触发的情况：需确认 root `package.json` 的 `dev` 脚本能正确跨盘 `cd frontend`
- Rust 编译首次耗时 5-15 min，Tauri CLI 在此期间仍会显示 "Waiting..." — 非卡死，属正常
