# OmniMD 产品升级需求文档

## 1. 项目概述

### 1.1 产品名称
OmniMD - Anything to Markdown

### 1.2 产品定位

OmniMD 是一个**免费、本地运行、隐私优先的文档处理工具**，核心目标不是单纯“文件转 Markdown”，而是：

> **将任意文件/网页转换为高质量、AI 可直接使用的 Markdown。**

核心价值：

- 完全免费
- 本地处理，文件默认不上传服务器
- 支持批量处理
- 转换结果结构清晰、适合 AI / RAG / Obsidian 等场景
- 操作简单，尽量做到“拖进去就能用”
- 不依赖云端 API 即可完成基础转换

### 1.3 核心目标用户

第一阶段重点面向：

1. AI 重度用户：经常把 PDF、Word、网页、论文等交给 ChatGPT / Claude / NotebookLM
2. Obsidian / 知识管理用户
3. 开发者、AI Agent、RAG 用户
4. 需要批量处理本地文档的办公用户

---

# 2. 当前版本基础

当前 MVP 已经具备：

- React 18
- TypeScript
- Tauri 2.x
- Tailwind CSS
- 基于 Firecrawl anydoc
- Windows 桌面客户端
- DOCX / PDF / PPTX / XLSX / EPUB / CSV / TXT / HTML / RTF / ODT 等格式
- 单文件转换
- 批量处理
- 并发数量设置
- Markdown 编辑/查看
- Markdown 预览
- 保存 `.md`
- 打开输出文件夹
- 转换历史
- 暂停/继续/取消批量任务
- 深色/浅色/自动主题
- 本地输出目录选择

**原则：不要推倒重做现有架构和 UI。应在当前 MVP 上迭代。**

---

# 3. 产品战略

OmniMD 后续不定位为：

> “又一个 PDF/Word 转 Markdown 工具”

而定位为：

> **本地 AI-ready 文档预处理工具**

核心流程：

```text
文件 / 文件夹 / URL
        ↓
      OmniMD
        ↓
高质量结构化 Markdown
        ↓
复制 / 保存 / Obsidian / AI / RAG / Git
```

---

# 4. P0：转换质量升级

这是整个项目最高优先级。

## 4.1 目标

输出 Markdown 必须尽可能保留原始文档的语义结构，而不是简单文本抽取。

优先保证：

- 标题层级
- 正文段落
- 有序/无序列表
- 表格
- 引用
- 超链接
- 图片
- 代码块
- 脚注（能力允许时）
- 文档章节顺序

避免：

- 页眉进入正文
- 页脚进入正文
- 目录重复
- 段落顺序错误
- 表格严重错位
- 多余空行
- Markdown 层级混乱
- 无意义的乱码
- 图片全部丢失

## 4.2 Markdown 清理

在 anydoc 基础转换结果上增加后处理 Pipeline：

```text
原始文件
→ anydoc
→ Markdown Normalize
→ Markdown Cleanup
→ Asset Processing
→ AI Ready Formatter（可选）
→ 最终 Markdown
```

需要处理：

- 连续空行
- 标题层级异常
- 列表格式异常
- 多余 HTML
- 空表格
- 乱码字符
- 重复内容
- 无意义的分页符

---

# 5. P0：图片与附件资产管理

这是 OmniMD 与普通 Markdown 转换器拉开差距的重要功能。

## 5.1 输出结构

例如输入：

```text
report.docx
```

输出：

```text
report/
├── report.md
└── assets/
    ├── image-001.png
    ├── image-002.png
    └── image-003.png
```

Markdown 使用相对路径：

```markdown
![图片描述](assets/image-001.png)
```

## 5.2 要求

- 图片尽可能提取原始文件
- 图片统一放入 `assets`
- Markdown 使用相对路径
- 不允许默认生成失效的绝对路径
- 文件名必须稳定
- 批量转换时不同文件的 assets 不能互相覆盖
- 同名图片需要自动处理冲突

## 5.3 后续扩展预留

未来允许增加：

- 图片 OCR
- 图片描述
- 图片压缩
- 图片格式转换

但本阶段不要因为 AI 图片处理阻塞基础功能。

---

# 6. P0：AI Ready Markdown

新增一个输出模式：

## Standard Markdown

保持传统 Markdown 转换逻辑。

## AI Ready Markdown

针对 AI 阅读、知识库、RAG 优化。

目标：

```text
原始文档
↓
结构化 Markdown
↓
更适合 LLM 阅读
```

需要考虑：

- 清理无意义格式
- 优化标题层级
- 保持章节结构
- 保留表格
- 保留重要链接
- 图片使用相对路径
- 删除明显的页眉/页脚噪声
- 可选生成目录
- 可选生成文档元信息

---

# 7. P1：文件夹递归批处理

现有“批处理”升级为：

> **文件 + 文件夹 + 文件夹递归**

## 7.1 支持

用户可以：

```text
拖入文件
```

也可以：

```text
拖入文件夹
```

程序自动扫描：

```text
Documents/
├── report.pdf
├── meeting.docx
├── slides.pptx
└── 2026/
    ├── a.pdf
    └── b.docx
```

并处理所有支持的文件。

## 7.2 要求

- 默认递归扫描
- 保留原始目录结构
- 支持过滤扩展名
- 支持跳过不支持文件
- 支持失败重试
- 支持暂停/继续/取消
- 支持并发数
- 完成后显示成功/失败数量

例如：

```text
output/
├── report/
│   ├── report.md
│   └── assets/
├── meeting/
│   ├── meeting.md
│   └── assets/
└── 2026/
    ├── a/
    │   └── a.md
    └── b/
        └── b.md
```

---

# 8. P1：URL → Markdown

现有“粘贴链接地址”升级为正式功能。

## 8.1 使用流程

用户输入：

```text
https://example.com/article
```

点击：

> 转换

输出：

```text
article.md
```

## 8.2 目标

尽量获取页面核心正文，而不是简单抓 HTML。

尽可能移除：

- 导航
- 广告
- Cookie Banner
- Footer
- 无关侧边栏
- 评论区域

尽可能保留：

- 标题
- 正文
- H1/H2/H3
- 列表
- 图片
- 表格
- 链接

## 8.3 网络隐私

URL 转换与本地文件转换应明确区分。

界面需要提示：

> URL 转换需要访问网络；本地文件转换不会上传文件。

不要在没有必要的情况下把本地文件发送到远程服务。

---

# 9. P1：Obsidian 输出模式

增加：

> Obsidian

输出模式。

目标目录：

```text
Knowledge/
├── article.md
└── assets/
    ├── image-001.png
    └── image-002.png
```

Markdown 需要使用适合本地知识库的相对路径。

可选生成 YAML Frontmatter：

```yaml
---
title: 示例文章
source: https://example.com
created: 2026-08-15
tags:
  - imported
---
```

第一阶段 Frontmatter 不要过度复杂。

至少支持：

- title
- source
- created
- tags

---

# 10. P1：Windows 右键菜单

这是重点功能。

安装 OmniMD 后，在 Windows 文件管理器中：

右键文件：

> 使用 OmniMD 转换为 Markdown

右键文件夹：

> 使用 OmniMD 批量转换为 Markdown

行为：

```text
用户右键
↓
OmniMD
↓
后台启动任务
↓
输出 Markdown
```

要求：

- 支持单文件
- 支持多文件
- 支持文件夹
- 支持打开 OmniMD 主窗口查看任务
- 失败时给出明确提示

优先支持 Windows，不要求第一阶段兼容 macOS/Linux 的系统右键菜单。

---

# 11. P1：默认输出目录和快速导出

改善导出体验。

支持：

- 保存 `.md`
- 保存 Markdown + assets
- 打开文件夹
- 复制 Markdown

新增快捷操作：

```text
复制 Markdown
复制纯文本
打开输出目录
重新转换
```

---

# 12. P2：文件夹监控

增加可选：

> 监控文件夹

示例：

```text
Knowledge/
```

OmniMD 自动监控。

当：

```text
Knowledge/report.pdf
```

新增或修改时：

自动生成：

```text
Knowledge/report.md
```

## 12.1 要求

- 用户主动开启
- 支持暂停
- 支持删除文件时是否同步删除 `.md`
- 防止无限循环
- 修改 Markdown 不应反向触发 PDF 转换
- 对频繁变化文件做 debounce

这项功能属于高级能力，不应该阻塞正式版发布。

---

# 13. P2：CLI

未来增加：

```bash
omnimd report.pdf
```

或者：

```bash
omnimd ./documents --recursive
```

示例：

```bash
omnimd ./docs --output ./markdown
```

用途：

- 开发者
- AI Agent
- 自动化脚本
- RAG pipeline
- CI/CD

CLI 与桌面端应共享核心转换逻辑。

第一阶段可以只设计架构，不必立刻完成全部 CLI 功能。

---

# 14. P2：AI 功能

AI 只能作为增强层，不允许成为基础转换功能的强依赖。

## 14.1 可选 AI 优化

增加按钮：

> AI 优化

可处理：

- 标题层级修复
- 内容清理
- 生成摘要
- 自动 tags
- 自动目录
- Frontmatter
- 图片描述
- OCR 后处理

## 14.2 原则

基础转换：

```text
不需要 API Key
不需要联网
不需要 AI
免费
```

AI 功能：

```text
用户主动启用
```

并明确提示可能需要 API / 网络。

---

# 15. UI/UX 调整

现有 UI 已经比较成熟，不建议大改视觉风格。

重点优化信息架构。

## 15.1 首页主标题

当前：

> 将任意文件转为 Markdown

建议：

> **把任何文件变成 AI 可用的 Markdown**

副标题：

> PDF · Word · PowerPoint · Excel · EPUB · HTML · URL

增加卖点：

```text
✓ 本地处理
✓ 隐私保护
✓ 完全免费
✓ 批量转换
```

## 15.2 首页核心入口

中心拖放区域应突出：

> 拖入文件或文件夹

下面：

```text
或者选择文件
或者粘贴 URL
```

## 15.3 输出模式

首页/转换前提供：

```text
输出模式

○ Standard
○ AI Ready
○ Obsidian
```

默认：

> AI Ready

但需要允许用户修改。

---

# 16. 转换结果页面优化

当前双栏：

```text
Markdown | Preview
```

继续保留。

底部建议：

```text
复制 Markdown
保存 .md
打开文件夹
导出 Obsidian
重新转换
```

增加：

```text
文件信息
转换耗时
原始大小
输出大小
图片数量
表格数量
警告数量
```

例如：

```text
转换完成

耗时：2.3 秒
图片：12
表格：8
警告：0
```

---

# 17. 错误处理

不能只显示：

> 转换失败

必须明确：

```text
转换失败

文件：example.pdf

原因：
PDF 内容无法正常解析

建议：
1. 检查文件是否损坏
2. 尝试开启 OCR
3. 使用其他版本文件
```

如果只部分失败：

```text
100 个文件
✓ 96 成功
⚠ 2 有警告
✕ 2 失败
```

允许：

> 重新转换失败项

---

# 18. 性能要求

目标：

- 本地转换速度快
- 支持多个文件并发
- 大文件不能轻易卡死 UI
- 转换过程不能阻塞主线程
- 任务状态必须实时更新
- 内存使用可控

并发数：

```text
1 ~ CPU 推荐值
```

默认值可以继续保留 4。

对于大型 PDF / PPTX / XLSX，应避免一次性把所有内容长期驻留内存。

---

# 19. 隐私要求

这是产品核心卖点。

应用需要明确说明：

> **本地文件转换默认完全在本机完成，文件不会上传到 OmniMD 服务器。**

除 URL / 用户主动启用 AI 功能外，不进行远程上传。

不要偷偷收集：

- 文件内容
- 文件名
- 文件路径
- 文档正文

统计功能未来如果增加，应默认关闭或采用匿名聚合数据。

---

# 20. 产品设置

现有设置页继续保留并增强：

### 外观

- 浅色
- 深色
- 自动

### 转换

- 默认并发数
- 默认输出模式
- 默认输出目录
- 是否递归处理文件夹
- 是否保留原目录结构

### AI

- AI 功能开关
- API 配置
- 是否允许联网

### 隐私

增加：

> 本地处理说明

明确告诉用户：

```text
本地文件不会上传。

只有以下功能可能访问网络：
- URL 转换
- 用户主动启用的 AI 功能
```

---

# 21. 未来暂不做

以下功能暂时不要投入大量开发资源：

- 云端账号体系
- 用户注册登录
- 在线文件存储
- 订阅收费体系
- 团队协作
- 在线编辑器
- 社交功能
- 复杂 AI 聊天
- 自建 RAG 平台
- 复杂 Notion 双向同步
- 大规模企业后台

当前阶段目标是：

> **先让单机工具本身足够优秀，让用户愿意每天使用。**

---

# 22. 开发优先级

## P0：必须先完成

### P0-1
转换质量优化

### P0-2
图片 / assets 管理

### P0-3
AI Ready Markdown

### P0-4
文件夹递归批处理

---

## P1：紧接着完成

### P1-1
URL → Markdown

### P1-2
Obsidian 输出

### P1-3
Windows 右键转换

### P1-4
导出/复制体验优化

### P1-5
错误信息和任务状态优化

---

## P2：后续增强

### P2-1
文件夹监控

### P2-2
CLI

### P2-3
AI 优化

### P2-4
OCR

### P2-5
更多第三方知识库集成

---

# 23. 核心产品闭环

必须确保下面这条路径非常顺：

```text
打开 OmniMD
↓
拖入文件 / 文件夹 / URL
↓
选择输出模式
↓
开始转换
↓
显示实时进度
↓
转换完成
↓
Markdown + assets
↓
一键复制 / 保存 / 打开文件夹 / 导入 Obsidian
```

目标：

> **新用户第一次使用 30 秒内完成一次成功转换。**

---

# 24. 完成标准

本轮升级完成后，用户应该可以：

### 场景 1：单文件

```text
拖入 PDF
→ AI Ready
→ 转换
→ 得到 report.md + assets/
```

### 场景 2：文件夹

```text
拖入 documents/
→ 自动递归
→ 保留目录结构
→ 全部生成 Markdown
```

### 场景 3：网页

```text
粘贴 URL
→ 获取干净正文
→ 输出 Markdown
```

### 场景 4：Obsidian

```text
PDF
→ Obsidian 模式
→ md + assets + frontmatter
```

### 场景 5：Windows

```text
右键 PDF
→ 使用 OmniMD 转换
→ 自动输出 Markdown
```

---

# 25. AI Agent 开发原则

开发过程中遵循以下原则：

1. **不要推翻当前已有实现，优先增量开发。**
2. **优先复用 anydoc 能力，避免重复实现底层文件解析。**
3. 将核心转换逻辑与 UI 解耦。
4. 转换 Pipeline 应模块化，方便未来加入 OCR、AI、更多格式。
5. 所有转换任务必须可观察、可取消、可重试。
6. 所有输出路径必须安全处理，防止路径穿越、文件覆盖和目录冲突。
7. 本地文件默认不联网。
8. AI 能力必须是可选增强，不影响基础转换。
9. UI 不要为了增加功能而变得复杂。
10. 每完成一个 P0/P1 功能，都应进行真实文件回归测试。

---

# 26. 推荐架构

建议逻辑结构：

```text
UI Layer
   ↓
Task Manager
   ↓
Conversion Service
   ↓
Parser / anydoc
   ↓
Markdown Pipeline
   ├── Normalize
   ├── Cleanup
   ├── Asset Manager
   ├── AI Ready Formatter
   └── Obsidian Formatter
   ↓
Exporter
   ├── .md
   ├── assets/
   └── Obsidian
```

URL 单独：

```text
URL
↓
Web Fetch / Extraction
↓
Markdown Pipeline
↓
Exporter
```

未来 CLI：

```text
CLI
 ↓
同一套 Conversion Service
```

---

# 27. 最终产品愿景

OmniMD 最终不是：

> PDF → Markdown

而是：

> **Anything → AI-ready Markdown**

即：

```text
PDF
DOCX
PPTX
XLSX
EPUB
HTML
CSV
TXT
URL
Folder
        ↓
      OmniMD
        ↓
Clean + Structured + AI Ready Markdown
        ↓
ChatGPT
Claude
NotebookLM
Obsidian
Cursor
RAG
Git
AI Agents
```

产品核心竞争力：

> **免费 + 本地 + 隐私 + 批量 + 高质量 + AI Ready + 极低使用门槛**

开发优先级必须始终围绕这个核心价值，而不是单纯追求“支持更多格式”。