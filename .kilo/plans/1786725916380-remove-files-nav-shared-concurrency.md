# 移除"文件"侧栏项 + 并发数全局共享

## 背景

用户反馈两点：
1. 侧栏顶部"文件"项是纯冗余——`active={false}` 写死、`onClick` 与紧邻的"首页"完全相同（`setPage("home")`），代码里不存在任何"文件"专属页面 → 删除
2. 并发数（首页/批处理各一个输入框）经核查是真功能：后端 `pipeline::convert_batch`（`FuturesUnordered` 滑动窗口）用它控制**同时转换的文件数**，不是文件数量上限；但 **BatchPage 顶栏的并发输入框是死的**——`concurrency` state 只喂给自己的 `<Input>`，从未传给任何转换调用 → 提升为全局共享，让两处输入框作用于同一值

## 前置约束（执行者必读）

- **本计划必须用 Agent Manager local 模式执行，禁止 worktree**：工作区有大量未提交改动（UI 改版 + 自绘标题栏 + capability 权限），git worktree 基于最近 commit 分支，会丢失这些未提交工作
- 纯前端改动，不触发 Rust 重编译；若用户 `pnpm tauri dev` 正在运行，vite HMR 自动生效（若样式不更新，清 `frontend/node_modules/.vite` 后重启 dev）
- 只改工作区文件，**不要提交**（用户未要求）

## 改动明细（4 个文件）

### 1. `frontend/src/App.tsx`
- **(a)** lucide import 去掉 `FolderOpen,`（当前第 4 行）
- **(b)** 删除整个"文件"侧栏块（当前约 86-93 行，位于 `<aside>` 开标签与 `<nav>` 之间）：
  ```tsx
        <div className="mb-2 px-1 flex-shrink-0">
          <SidebarNavItem
            icon={<FolderOpen size={16} />}
            label={t("nav.files")}
            active={false}
            onClick={() => setPage("home")}
          />
        </div>
  ```

### 2. `frontend/src/store/useTaskStore.ts`
- **(a)** `TaskStore` 接口新增两个字段：
  ```ts
  concurrency: number;
  setConcurrency: (n: number) => void;
  ```
- **(b)** store 对象 `currentResult: null,` 之后新增 `concurrency: 4,`
- **(c)** `selectTask` 方法之后（`}));` 之前）新增：
  ```ts
  setConcurrency: (n) => set({ concurrency: n }),
  ```

### 3. `frontend/src/pages/HomePage.tsx`
- **(a)** `useTaskStore()` 解构处（约 29-36 行）加入 `concurrency, setConcurrency`
- **(b)** 删除本地 state 行（约第 39 行）：
  ```tsx
  const [concurrency, setConcurrency] = useState(4);
  ```
- `handleFiles` 依赖数组里已有 `concurrency`，保留不动；`useState` 仍在用（`outputDir`），import 保留

### 4. `frontend/src/pages/BatchPage.tsx`
- **(a)** 删除第 1 行 `import { useState } from "react";`
- **(b)** `useTaskStore()` 解构（`const { tasks, cancelTask, clearCompleted } = useTaskStore();`）加入 `concurrency, setConcurrency`
- **(c)** 删除本地 state 行（约第 23 行）：
  ```tsx
  const [concurrency, setConcurrency] = useState(4);
  ```

## 涉及文件
- `frontend/src/App.tsx`（改）
- `frontend/src/store/useTaskStore.ts`（改）
- `frontend/src/pages/HomePage.tsx`（改）
- `frontend/src/pages/BatchPage.tsx`（改）

不负责任务：Rust 后端（`src/*.rs`）、capabilities、i18n、（可选项）批量转换期间逐文件阶段进度事件——本次不做

## 验证
1. `cd D:\project\OmniMD\frontend && pnpm build` 通过（tsc + vite）
2. `grep` 确认：`nav.files` 与 `FolderOpen` 在 `App.tsx` 无残留；`HomePage.tsx` / `BatchPage.tsx` 中 `concurrency`/`setConcurrency` 只接受来自 `useTaskStore` 的值（无本地 `useState(4)`）
3. 运行中应用目测：侧栏只剩 首页/转换/批处理/设置；首页与批处理页的并发输入框改任意值后保持同步