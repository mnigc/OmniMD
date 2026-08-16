# 导航图标/顺序 + 预览页文件路径宽度

## 改动 1：预览导航图标换为眼睛

**文件：** `frontend/src/App.tsx`

- 第 3 行 import：`ArrowRightLeft` → `Eye`
- 第 97 行 nav item：`<ArrowRightLeft size={16} />` → `<Eye size={16} />`

## 改动 2：预览导航项移到转换历史下方

**文件：** `frontend/src/App.tsx` 第 89-108 行

当前顺序：Home → Convert → History

改为：Home → History → Convert（预览）

即交换 `<SidebarNavItem>` for `t("nav.convert")` 和 `t("nav.history")` 两个块的位置。

## 改动 3：预览页文件路径显示区域加宽

**文件：** `frontend/src/pages/ConvertPage.tsx` 第 254 行

当前：
```tsx
<span className="text-sm font-medium truncate max-w-72">
```

改为（放宽宽度限制）：
```tsx
<span className="text-sm font-medium truncate max-w-[480px]">
```

同时把外层容器 `max-w-72` 也检查一遍（第 252 行 `<div className="flex items-center gap-2 h-8 px-3 bg-muted rounded-md">`），目前外层没有限制宽度，所以只需改内层 `max-w-72`。

## 验证

`pnpm tauri dev`，确认：
1. 侧边栏预览项图标是眼睛
2. 导航顺序：首页 → 转换历史 → 预览 → 设置
3. 预览页顶部文件路径显示更宽，长路径不再被截断