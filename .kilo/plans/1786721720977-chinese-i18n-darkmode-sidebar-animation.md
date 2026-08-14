# OmniMD 三项功能改造计划

## 目标

1. 全应用 UI 文案中文化（标题/Logo 除外），为后续多语言做准备
2. 支持深色/浅色/自动三档主题切换
3. 侧边栏显示/隐藏添加平滑过渡动画

---

## 需求 1：中文国际化

### 方案
- 创建 `src/i18n/` 目录，包含：
  - `locales/zh-CN.ts` — 中文翻译（当前全部文案）
  - `locales/en.ts` — 英文翻译（占位，与当前文案一致）
  - `index.ts` — i18n context，提供 `t(key)` 函数和 `locale` 状态
- 所有硬编码 UI 文案替换为 `t("path.to.key")` 调用
- 默认语言 `zh-CN`，后续通过语言选择器切换

### 需要翻译的文案位置
| 文件 | 文案位置 |
|------|---------|
| `App.tsx` | navItems labels, 顶部 "Converting:", 侧边 "Files", "Phase 1 MVP", "anydoc + Tauri 2.x", Settings 占位文案 |
| `HomePage.tsx` | H1/H2 标题, placeholder, 按钮文案, "Recent Conversions", 统计标签, "Converting..." |
| `ConvertPage.tsx` | "No file to preview", viewMode title, "Markdown", "Preview", 底部按钮文案 |
| `BatchPage.tsx` | "Batch Queue", 按钮文案, 状态标签 |
| `DropZone.tsx` | 拖拽提示, 支持格式说明, 选中提示 |
| `TaskItem.tsx` | statusLabel（Pending/Processing/Completed/Failed/Cancelled 翻译） |
| `MarkdownPreview.tsx` | "No markdown content to preview" |

### 文件
1. 新建 `src/i18n/locales/zh-CN.ts`
2. 新建 `src/i18n/locales/en.ts`
3. 新建 `src/i18n/index.ts`（I18nProvider + useI18n）
4. 修改 `main.tsx`，用 `I18nProvider` 包裹 `App`
5. 逐个修改上述各文件，替换文案为 `t()` 调用

---

## 需求 2：深色模式（深色/浅色/自动）

### 方案
- CSS 变量定义颜色 token，`data-theme` 属性控制：
  - `data-theme="light"` → 浅色变量
  - `data-theme="dark"` → 深色变量
  - `data-theme="auto"` → 监听 `prefers-color-scheme` 媒体查询，跟随系统
- Tailwind 配置启用 `darkMode: "class"`，在 `<html>` 上加 `dark` class
- 主题选择器放在 App header 右侧，三个选项：浅色 / 深色 / 自动
- 主题选择持久化到 `localStorage`
- 全局监听 `prefers-color-scheme` change 事件，auto 模式实时跟随

### 颜色策略
- 现有所有 `bg-slate-*` / `text-slate-*` 等硬编码颜色替换为 `var(--xxx)` 引用
- 在 `index.css` 中定义两套变量：
  ```
  :root { /* light tokens */ }
  :root[data-theme="dark"] { /* dark tokens */ }
  ```
- 关键 tokens: `--bg`, `--bg-secondary`, `--border`, `--text`, `--text-muted`, `--hover-bg`

### 文件
1. 修改 `index.css` — 定义 CSS 变量两套主题，`.prose` 样式改用变量
2. 修改 `tailwind.config.js` — `darkMode: "class"`
3. 新建 `src/lib/theme.ts` — 主题工具函数（applyTheme, getSystemTheme, 监听器）
4. 新建 `src/components/ThemeSelector.tsx` — 三选项切换器（可用单选按钮组或下拉菜单）
5. 修改 `App.tsx` — header 添加 ThemeSelector，移除硬编码 `bg-white` 改为变量
6. 逐个修改各页面文件，替换硬编码颜色为 CSS 变量类

---

## 需求 3：侧边栏过渡动画

### 方案
- 当前 `{sidebarOpen && <aside>}` 直接 mount/unmount，无动画
- 改为 `overflow-hidden` + `width` transition：
  - 打开：`width: 208px`（w-52）
  - 关闭：`width: 0`
- 使用 CSS transition：`width 250ms ease-out`
- aside 内内容用 `overflow-hidden` 防止关闭时溢出
- `min-w-0` 防止 flex 子项无法缩到 0

### 文件
1. 修改 `App.tsx` — aside 改为始终渲染，`style={{ width: sidebarOpen ? 208 : 0 }}` + transition class

---

## 实现顺序

1. **国际化** — 先建 i18n 框架，再替换所有文案
2. **深色模式** — 定义 CSS 变量，再切换各页面颜色引用
3. **侧边栏动画** — 最后改，改动最小

---

## 验证步骤

1. `cd frontend && pnpm dev` 启动应用
2. 确认所有可见 UI 文案为中文
3. 切换深色/浅色/自动三种模式，确认颜色正常切换
4. 系统切换深色/浅色时，自动模式跟随变化
5. 点击侧边栏切换按钮，确认 250ms 平滑过渡，无抖动

---

## 风险与注意事项

- 深色模式需注意 `.prose` 内的颜色也要跟随主题
- `data-theme="auto"` 下需要同时监听系统变化和应用初始化时的状态
- 侧边栏动画要注意 `overflow` 处理，避免关闭时内容闪现