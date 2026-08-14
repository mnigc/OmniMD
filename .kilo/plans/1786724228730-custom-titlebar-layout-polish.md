# OmniMD 自绘标题栏 + 排版精修计划

## 背景与目标

用户反馈两点：
1. 顶部是最新的原生 Windows 标题栏（最小化/最大化/关闭是系统组件），观感和应用脱节
2. 整体排版不够精致——左中右内容高度不齐（尤其 Home 底部操作行）、各处字号/宽度/控件高度不一致

已确认的三个决策：
- **标题栏形式**：与应用栏合并成一条（像 VS Code / Linear），不额外占行高
- **精修范围**：系统性对齐 + 建规范（所有页面），重点重构 Home
- **字体**：真正加载 Inter（`@fontsource-variable/inter`），覆盖全部字重

## 决策与边界

- 自绘窗口控件仅在 Windows 显示（`navigator.userAgent` 含 "Windows"）；macOS/Linux 开发时隐藏控件，拖拽区域属性保持无害
- 拖拽区域用 Tauri v2 的 `data-tauri-drag-region` 属性；header 上的交互元素（侧栏折叠按钮、主题下拉触发器）不加该属性
- 双击拖拽区自动最大化/还原由 Tauri 原生处理
- Snap Layout 悬停飞出的功能（Win11 悬停最大化按钮）在 `decorations: false` 下不可用，接受此限制；Aero Snap 拖边停靠仍可用
- 不动 Rust 后端逻辑；仅 `tauri.conf.json` 窗口配置 + 前端

## 实施步骤

### Step 1 — 窗口配置：关原生标题栏
`tauri.conf.json`（窗口数组内）：
```json
{
  "title": "OmniMD - Anything to Markdown",
  "width": 1200, "height": 800, "minWidth": 800, "minHeight": 600,
  "decorations": false,
  "shadow": true
}
```
- `decorations: false` 移除系统标题栏
- `shadow: true` 保持 Windows 窗口阴影（默认即 true，显式声明防回归）
- 保留默认 `resizable`（边缘拖拽缩放不受影响）；此改动需重启 `pnpm tauri dev`（触发 Rust 重编译）

### Step 2 — 依赖
```bash
cd frontend
pnpm add @fontsource-variable/inter
pnpm add @tauri-apps/api     # 从 devDependencies 移入 dependencies（窗口控制运行时导入，更规范）
```

### Step 3 — 引入 Inter 字体
- `frontend/src/main.tsx` 顶部：`import "@fontsource-variable/inter";`
- `tailwind.config.js` `fontFamily.sans` 首位改为 `"Inter Variable"`：
  ```js
  sans: ["Inter Variable", "PingFang SC", "Microsoft YaHei", "system-ui", "sans-serif"],
  ```
- 中文回落系统字体不变；拉丁字母/数字立即变精致

### Step 4 — 自绘窗口控件组件
新建 `frontend/src/components/WindowControls.tsx`：
- 用 `@tauri-apps/api/window` 的 `getCurrentWindow()`：`minimize()`、`toggleMaximize()`、`close()`
- 状态 `isMaximized`：挂载时 `win.isMaximized()` + `win.onResized()` 监听（配合 `toggleMaximize` 后图标切换）
- 三个按钮：lucide `Minus`(最小化)、`Square`(最大化)/`Copy`(还原)、`X`(关闭)，size≈14
- 样式：每个按钮 `h-full w-11`（Windows 惯例约 44-46px）、`flex items-center justify-center`、`text-muted-foreground`、hover `bg-muted/80`；关闭按钮 hover `bg-red-600 text-white`
- 选中态 focus-visible ring 同全项目规范
- 组件仅 Windows 渲染：
  ```ts
  const isWindows = navigator.userAgent.includes("Windows") || navigator.userAgent.includes("Win64");
  if (!isWindows) return null;
  ```
- `WindowControls` 不设置 `data-tauri-drag-region`

### Step 5 — App.tsx：header 变拖拽应用栏
- header 元素加 `data-tauri-drag-region` + `select-none`，高度保持 `h-12`
- 交互子元素（折叠按钮、ThemeSelector 触发器、转换状态 chip）**不加**该属性
- 最右侧渲染 `<WindowControls />`，置于 `ml-2` 后贴边
- 其余结构（Logo/标题/侧栏/版本尾）不动

### Step 6 — 建立统一布局规范（新建原语）
新建 `frontend/src/components/PageHeader.tsx`：
```tsx
interface PageHeaderProps {
  title: string; description?: string; actions?: React.ReactNode;
}
```
结构：`flex items-center justify-between gap-4 mb-6`，标题 `text-xl font-semibold tracking-tight`，描述 `text-sm text-muted-foreground`，右侧 `actions` 区。

统一约定（写入 plan，供实施对照）：
- 页面壳：`h-full flex flex-col`；内容区 `flex-1 min-h-0 overflow-auto`
- 页面内边距：`px-6 py-6`；工具栏/页头条 `px-4`或`px-6`、`gap-2`、高 h-9（主）/ h-8（紧凑）
- 标题层级：页面标题一律 `text-xl font-semibold tracking-tight`（弃用 3xl/2xl/xl 混用）；`text-xl` 以上仅用于统计大数字
- 内容容器：一律 `mx-auto w-full` 居中收紧——Home `max-w-6xl`、Batch `max-w-4xl`、Settings `max-w-2xl`
- 控件高度：按钮/输入框默认 `h-9`，紧凑 `h-8`，toolbar 内统一
- 微观字号：`text-[10px]` 全部改为 `text-xs`（或结构性 `text-[11px] uppercase tracking-wide` 标签）；统计数字加 `tabular-nums`
- 数字用 `text-success` / `text-destructive` 语义色（已有 token）
- 所有 flex 子项防溢出：必要时 `min-w-0` + `truncate`

### Step 7 — 重构 HomePage（重点修"左中右不齐"）
- 外层内容加 `max-w-6xl mx-auto w-full`
- **顶部表单行**：输出目录（`Label`+`Input`，flex-1）与 并发数（`Label`+`Input w-24`）并为一行，两个控件同高 h-9，label 均在输入框上方（`flex flex-wrap gap-x-6 gap-y-4`）
- **DropZone 占满左栏剩余高度**：左栏 `flex flex-col`，`<DropZone className="flex-1 min-h-[220px]" />`（DropZone 需支持透传 className，改其根元素合并 `cn`）
- **底部操作行**：`添加文件` Button + `粘贴 URL` Input（均 h-9）、右侧无并发数了 → 不再混高；行内 `items-center gap-3`
- **右栏 Card**：ScrollArea 由 `max-h-72` 改为 `flex-1 min-h-0`（Card 需配合 `h-full flex flex-col`），列表真正占满；CardFooter 统计 `py-3`、数字 `text-2xl font-bold tabular-nums`、成功/失败 `text-success`/`text-destructive`
- 空状态居中图标 + 两行文字不变

### Step 8 — 对齐 Convert / Batch / Settings
- **ConvertPage**：顶部条 chip 与 SegmentedControl 高度统一（chip `h-8 items-center`、SegmentedControl `size="md"`）；左/右面板头 `h-9`；底部操作栏按钮统一 `size="sm"` + `gap-2`；用 PageHeader 规范调整页面标题（此页可加页头或保持无标题的空状态引导）
- **BatchPage**：标题改用 `PageHeader`；内容容器保持 `max-w-4xl mx-auto`；底部统计 pills 统一 `gap-2 px-2.5 py-1` + 数字 `tabular-nums`
- **SettingsPage**：改用 `PageHeader`（title + 描述）；两 Card 间距 `gap-6`；InfoRow 行高统一 `py-2.5`，分隔保持 `divide-y divide-border`
- **TaskItem.tsx**：字号统一（`text-xs`，去掉碎片化尺寸），进度百分比数字 `tabular-nums`

### Step 9 — 收尾与回归
- 全局检查残留 `text-[10px]`、混用字号、`dark:` 残留（仅 ThemeSelector 图标合法）
- `pnpm build` 通过（tsc + vite）

## 涉及文件

**修改**
- `frontend/../tauri.conf.json`（decorations/shadow）
- `frontend/src/main.tsx`（fontsource import）
- `frontend/tailwind.config.js`（Inter Variable 字体栈）
- `frontend/src/App.tsx`（header 拖拽区 + WindowControls）
- `frontend/src/pages/{HomePage,ConvertPage,BatchPage,SettingsPage}.tsx`
- `frontend/package.json` / `pnpm-lock.yaml`
- `frontend/src/components/{DropZone,TaskItem}.tsx`（className 透传 / 字号统一）

**新增**
- `frontend/src/components/WindowControls.tsx`
- `frontend/src/components/PageHeader.tsx`

**不动**
- Rust 后端（`src/*.rs`）、`theme.ts`/`index.html` 内联脚本、store、api

## 风险与注意

1. **Rust 需重编译**：`decorations:false` 后必须重启 `pnpm tauri dev`
2. **拖拽区与交互元素**：header 整个拖拽会导致子元素不可点击/拖窗——务必只对非交互区加 `data-tauri-drag-region`；双击最大化由 Tauri 处理，不要自制
3. **窗口阴影**：关闭原生标题栏后若边界发虚，检查 `shadow: true`（Win11 上 DWM 会自带圆角）
4. **Snap Layout**：自定义最大化按钮无 Win11 悬停飞层，接受为已知限制；Aero Snap 拖边不受影响
5. **Stale dev server**：上一轮教训，重启后如样式不更新，先清 `node_modules/.vite` 再重启
6. **字体生效**：确认 Inter Variable 被 Tailwind 编译进 font-family（看 dev tool computed style），CJK 回落正常

## 验证

1. `cd frontend && pnpm build` 通过
2. `pnpm tauri dev` 后手动检查：
   - 无原生标题栏；header 整条可拖窗；双击最大化/还原；最小化/最大化/关闭三按钮可用（关闭 hover 变红）
   - 边缘拖拽缩放正常；窗口有阴影
   - Home 底部行三元素同高对齐；左栏 DropZone 填满、右栏列表占满、统计不悬空
   - 三页面标题/容器宽度/控件高度一致
   - 浅色/深色/自动三档正常（重点：新布局下无混色）
   - 1200×800 与最大化两档目测无错位