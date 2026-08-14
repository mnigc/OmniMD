# OmniMD UI 整体重构计划

## 背景与目标

当前应用使用 Tailwind v3 + React 18 + 手写 className，颜色/间距/交互不统一，导致：
- 大量 `text-slate-900 dark:text-slate-100` 散落，深色模式反复出问题
- Button/Input/Card 每个页面手写一遍，样式不一致
- 排版层级、焦点态、空状态粗糙

目标：**引入 shadcn/ui 风格组件原语 + 语义化设计 token + 统一排版**，实现"一套 token 走天下"，彻底根治深色模式问题，整体观感对齐 Linear / Raycast / VS Code 级生产力工具。

---

## 技术决策

### 依赖情况（现状）
| 依赖 | 状态 |
|------|------|
| `clsx` / `tailwind-merge` | ✅ 已有（shadcn 底层三件套之二） |
| `lucide-react` | ✅ 已有（shadcn 图标依赖） |
| `tailwindcss` 3.4 | ✅ 已有（v3，非 v4） |
| `class-variance-authority` (cva) | ❌ 需新增 |
| `@radix-ui/react-slot` | ❌ 需新增 |
| `@radix-ui/react-tooltip` | ❌ 需新增 |
| `@radix-ui/react-dropdown-menu` | ❌ 需新增 |
| `@radix-ui/react-separator` | ❌ 需新增 |
| `@radix-ui/react-scroll-area` | ❌ 需新增 |
| `@radix-ui/react-label` | ❌ 需新增 |

### 方案：手写 shadcn 风格原语（不走 CLI）
- 最新 shadcn CLI 默认面向 Tailwind v4，与项目 v3 存在版本摩擦
- 手写 `src/components/ui/*` 组件（遵循 shadcn 的 cva + tailwind-merge 写法），完全可控、无网络依赖、符合现有依赖
- 仅安装上表列出的最小依赖

### 主题机制收敛（根治深色模式）
- **关键简化**：语义 token 只挂在 `.dark` class 上（shadcn 约定），删除 `:root[data-theme="dark"]` 这一条 CSS 触发路径
- `applyTheme()` 已保证 `data-theme` 与 `.dark` class 永远同一步（上轮已修复），所以 `.dark` 单一来源即可驱动一切
- `index.html` 内联脚本不变（已同时设置 data-theme + dark class，兼容）
- 组件里一律 `bg-background` / `text-foreground` / `text-muted-foreground` / `border-input` / `bg-primary`，**删除所有 `dark:` 变体**

---

## 实施步骤

### Step 1 — 安装依赖
```bash
cd frontend
pnpm add class-variance-authority @radix-ui/react-slot @radix-ui/react-tooltip @radix-ui/react-dropdown-menu @radix-ui/react-separator @radix-ui/react-scroll-area @radix-ui/react-label
```

### Step 2 — 设计 token 迁移（index.css）
将现有变量改为 shadcn 语义 token（HSL 空格分隔三元组，支持透明度修饰符）：

```css
@tailwind base;
@tailwind components;
@tailwind utilities;

:root {
  --background: 0 0% 100%;
  --foreground: 222.2 84% 4.9%;
  --card: 0 0% 100%;
  --card-foreground: 222.2 84% 4.9%;
  --popover: 0 0% 100%;
  --popover-foreground: 222.2 84% 4.9%;
  --primary: 258 90% 62%;        /* 品牌 violet */
  --primary-foreground: 0 0% 100%;
  --secondary: 210 40% 96.1%;
  --secondary-foreground: 222.2 47.4% 11.2%;
  --muted: 210 40% 96.1%;
  --muted-foreground: 215.4 16.3% 46.9%;
  --accent: 258 94% 93%;         /* violet-100 */
  --accent-foreground: 263 70% 40%;
  --destructive: 0 84.2% 60.2%;
  --destructive-foreground: 0 0% 98%;
  --border: 214.3 31.8% 91.4%;
  --input: 214.3 31.8% 91.4%;
  --ring: 258 90% 62%;
  --success: 142 71% 45%;
  --warning: 38 92% 50%;
  --radius: 0.5rem;
}

.dark {
  --background: 222.2 84% 4.9%;      /* #0f172a 一脉的深蓝黑 */
  --foreground: 210 40% 98%;
  --card: 222.2 84% 4.9%;
  --card-foreground: 210 40% 98%;
  --popover: 222.2 84% 4.9%;
  --popover-foreground: 210 40% 98%;
  --primary: 258 90% 62%;
  --primary-foreground: 0 0% 100%;
  --secondary: 217.2 32.6% 17.5%;
  --secondary-foreground: 210 40% 98%;
  --muted: 217.2 32.6% 17.5%;
  --muted-foreground: 215 20.2% 65.1%;
  --accent: 263 70% 30%;
  --accent-foreground: 240 50% 90%;
  --destructive: 0 62.8% 30.6%;
  --destructive-foreground: 0 0% 98%;
  --border: 217.2 32.6% 17.5%;
  --input: 217.2 32.6% 17.5%;
  --ring: 258 90% 62%;
  --success: 142 69% 58%;
  --warning: 38 92% 50%;
}

@layer base {
  body {
    @apply bg-background text-foreground;
  }
}
```

同时保留/重构 `.prose` 样式，全部改用 token（`var(--foreground)` / `var(--muted-foreground)` / `var(--border)`）。

**删除**：`:root[data-theme="dark"]` 块、`body { color: var(--text) }` 等旧写法（被 `@apply bg-background text-foreground` 取代）。

### Step 3 — 更新 tailwind.config.js
```js
colors: {
  background: "hsl(var(--background))",
  foreground: "hsl(var(--foreground))",
  card: { DEFAULT: "hsl(var(--card))", foreground: "hsl(var(--card-foreground))" },
  popover: { DEFAULT: "hsl(var(--popover))", foreground: "hsl(var(--popover-foreground))" },
  primary: { DEFAULT: "hsl(var(--primary))", foreground: "hsl(var(--primary-foreground))" },
  secondary: { DEFAULT: "hsl(var(--secondary))", foreground: "hsl(var(--secondary-foreground))" },
  muted: { DEFAULT: "hsl(var(--muted))", foreground: "hsl(var(--muted-foreground))" },
  accent: { DEFAULT: "hsl(var(--accent))", foreground: "hsl(var(--accent-foreground))" },
  destructive: { DEFAULT: "hsl(var(--destructive))", foreground: "hsl(var(--destructive-foreground))" },
  border: "hsl(var(--border))",
  input: "hsl(var(--input))",
  ring: "hsl(var(--ring))",
  success: "hsl(var(--success))",
  warning: "hsl(var(--warning))",
},
borderRadius: { lg: "var(--radius)", md: "calc(var(--radius) - 2px)", sm: "calc(var(--radius) - 4px)" },
fontFamily: {
  sans: ["Inter", "PingFang SC", "Microsoft YaHei", "system-ui", "sans-serif"],
  mono: ["JetBrains Mono", "SFMono-Regular", "Menlo", "monospace"],
},
keyframes: 保留现有 spin/pulse/accordion + 新增 fade-in/fade-out
```

### Step 4 — 创建 UI 原语（`src/components/ui/`）
| 组件 | 说明 |
|------|------|
| `button.tsx` | cva variants：default(primary)、secondary、outline、ghost、destructive、link；sizes sm/default/lg/icon；支持 `asChild`（Radix Slot） |
| `input.tsx` | 统一高度 h-9、边框、focus ring |
| `label.tsx` | 表单 label |
| `card.tsx` | Card/CardHeader/CardTitle/CardDescription/CardContent/CardFooter |
| `badge.tsx` | variants：default/secondary/outline/destructive/success/warning — 用于任务状态 |
| `separator.tsx` | 垂直/水平分割线 |
| `tooltip.tsx` | 图标按钮提示（Radix TooltipProvider/Tooltip） |
| `dropdown-menu.tsx` | 设置/主题下拉（Radix DropdownMenu） |
| `scroll-area.tsx` | 可滚动容器（列表、预览） |
| `progress.tsx` | 进度条（任务进度） |
| `spinner.tsx` | 加载中（lucide Loader2 封装） |

所有组件统一：`focus-visible:ring-2 ring-ring ring-offset-2`、`transition-colors`、`disabled:opacity-50`、`disabled:pointer-events-none`。

### Step 5 — 重构 ThemeSelector → 下拉选择
- 用 DropdownMenu：触发器显示当前主题图标 + 文案（浅色/深色/自动），菜单三项各带图标 + label + 单选勾选态
- 移除紧巴巴的三按钮分段，header 更干净
- 交互逻辑复用现有 `useState(getStoredTheme)` + `applyTheme` + 系统监听

### Step 6 — 重构页面（逐个替换并删 dark: 变体）

**App.tsx（应用外壳）**
- Root：`bg-background text-foreground`
- Header：`bg-background/95 backdrop-blur border-b`，折叠按钮用 `Button variant="ghost" size="icon"`
- Sidebar：
  - 容器 `bg-muted/40`，宽度过渡保留（上轮动画不破坏）
  - 抽出 `SidebarNavItem`：激活态 = primary/10 底 + 左侧 2px 指示条 + primary 文字；非激活 = muted 文字 + hover:bg-muted
  - Footer 版本信息并入 settings 链接区
- Settings 页改为 `SettingsPage.tsx`（不再占位）：
  - Card：外观设置（主题三选一）
  - Card：关于（版本、技术栈、语言切换占位）

**HomePage.tsx**
- 标题用 `text-3xl font-bold tracking-tight` + `text-muted-foreground` 副标题
- 输出目录区：`flex gap-2` + `Button`(Browse) + `Input`(flex-1) + 文字 label
- 操作按钮：`Button`（Add Files = default/primary，带 FilePlus icon）
- 并发数：`Label` + `Input type="number"` 收窄宽度
- DropZone 增强：渐变描边、dragging 时 `scale-[1.01] border-primary bg-primary/5 shadow-lg`、focus-visible ring
- 最近转换：`Card` + CardHeader + TaskItem 列表 + `ScrollArea`；空状态图标 + 两行文字
- 底部统计：三个小块，数字用 `text-2xl font-bold`，标签用 `text-muted-foreground text-xs`；成功/失败用 `text-success`/`text-destructive`

**ConvertPage.tsx**
- viewMode 切换 → `SegmentedControl`（自制：三个 icon 按钮组，激活态 `bg-muted` + ring 或 primary），不再手写三元 px-3 class
- 编辑面板：`ScrollArea` 包 textarea 或直接 div 滚动；预览面板同
- 顶部文件名 chip：`bg-muted rounded-md` + FileDown icon
- 底部操作栏：`Separator` 分隔，次要按钮 `Button variant="outline"`，禁用项 `disabled opacity`（AI Optimize 保留 disabled）
- 空状态：居中 icon + 两行文字 + 引导按钮

**BatchPage.tsx**
- 头部：标题 + `Button variant="outline"`（Pause/Resume/Clear Done）+ `Button variant="destructive"`（Cancel All）+ concurrency Input
- 队列：每条 TaskItem 完整态
- 底部统计：`Badge` 或 `flex gap` + 图标 + 计数，替代裸数字；`Total` 用 muted
- 空状态：icon + 引导文字

**TaskItem.tsx**
- 状态 → `Badge`（Processing=secondary+spinner、Completed=success、Failed=destructive、Pending=cancelled=muted/warning）
- 文件名 + 扩展名 chip（`bg-muted text-muted-foreground rounded`）
- 进度条 → `Progress` 组件（value 0-100，颜色随状态）
- 图标按钮 → `Button variant="ghost" size="icon"` + Tooltip
- 错误信息：`text-destructive text-xs`

**DropZone.tsx**
- 结构保留，视觉升级：rounded-xl、icon 圆形容器 `bg-muted`、主文案 `text-foreground`、副文案 `text-muted-foreground`
- 选中提示：`Badge variant="success"`

**MarkdownPreview.tsx**
- 移除全部 `dark:` 变体，改用 token（`bg-muted`、`border-border`、`text-muted-foreground`、链接 `text-primary hover:underline`）
- `.markdown-body` 由 index.css 的 token 化 `.prose` 接管基础样式

### Step 7 — 排版与间距收尾
- 全局字体栈接入（tailwind fontFamily）
- 统一圆角（--radius + rounded-lg/md/sm 语义）
- 检查 padding/margin：页面统一 `p-6`、区块间距 `gap-4/6`、容器 max-width 收敛
- 添加全局 `@layer base` 的 smooth scroll、默认 `antialiased`

### Step 8 — 验证
```bash
cd frontend && pnpm build        # tsc + vite 通过
pnpm tauri dev                   # 手动过一遍
```
手动检查项：
1. 浅色 / 深色 / 自动 三档切换全部正常（重点：auto 跟随系统，无混色）
2. 全 app 无残留 `dark:` class 或 `slate-*：`假想检查 → `rg "dark:" src/` 应为 0（除了 guard 场景） 
3. 中文文案完整（i18n 测试）
4. 侧边栏动画不破坏
5. Convert / Batch / Home 全流程操作

---

## 涉及文件清单

**新增**
- `frontend/src/components/ui/{button,input,label,card,badge,separator,tooltip,dropdown-menu,scroll-area,progress}.tsx`
- `frontend/src/pages/SettingsPage.tsx`
- `frontend/src/components/SidebarNavItem.tsx`
- `frontend/src/components/SegmentedControl.tsx`

**修改**
- `frontend/src/index.css`（token 全面重写）
- `frontend/tailwind.config.js`（colors/fontFamily/radius/keyframes）
- `frontend/vite.config.ts`（如需路径别名 `@/`，按需添加；不强制）
- `frontend/src/App.tsx`
- `frontend/src/pages/{HomePage,ConvertPage,BatchPage}.tsx`
- `frontend/src/components/{ThemeSelector,DropZone,TaskItem,MarkdownPreview}.tsx`
- `frontend/src/i18n/locales/*.ts`（新增 settings 区块文案：外观/关于/语言占位）

**不动**
- Rust 后端、store、api、暗黑模式核心逻辑（theme.ts / index.html 内联脚本）

---

## 风险与注意

1. **`rg "dark:"` 清零**：重构后组件不应再有 `dark:` 变体；一切交给 token。若个别特殊场景（如 shadow）仍需变体，注释说明
2. **Radix 依赖引入**：确认 pnpm 安装成功；Tooltip/Dropdown 需 Provider 包裹（放 App 根）
3. **`.prose` token 化**：Markdown 表格/代码块在深色下必须可读，重点回归测试
4. **侧边栏动画**：重构 aside 时保留 `transition-all` 宽度切换逻辑
5. **Ops 版本**：`--radius` 值改动会全局影响圆角观感，重构中期截图确认

## 里程碑
1. M1：token + 原语 + 主题下拉（骨架跑通，深色问题根除）
2. M2：App 外壳 + Sidebar + Settings 页
3. M3：Home / Convert / Batch 三页 + TaskItem/DropZone
4. M4：排版字体收尾 + 全量回归