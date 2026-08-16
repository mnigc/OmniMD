# OmniMD 产品与技术需求文档
## 产品方向：本地 Markdown 工作台 + MinerU 文档解析引擎

版本：v1.0
用途：提供给 AI Agent 直接进行架构分析、开发与重构

---

# 1. 产品重新定位

OmniMD 不再把自己定位成：

> “又一个 PDF / Office 转 Markdown 工具”

也不与 MinerU 竞争底层文档解析能力。

新的产品定位：

> **一个精致、极简、本地、免费的 Markdown 工作台。**

一句话：

> **Drop anything. Get clean Markdown.**

中文：

> **拖入任何文件，得到干净的 Markdown。**

核心理念：

```text
任何内容
  ↓
Import
  ↓
MinerU / Native Parser
  ↓
高质量 Markdown
  ↓
编辑 / 阅读 / 搜索 / 收藏 / 管理
  ↓
本地 Markdown 知识库
```

转换只是入口。

长期价值来自：

> **Markdown 文件管理 + 阅读 + 编辑 + 搜索 + 本地知识库工作流。**

---

# 2. 为什么采用 MinerU

MinerU 当前官方项目已经针对 PDF、图片、DOCX、PPTX、XLSX → Markdown/JSON 做了完整文档解析，并覆盖 OCR、复杂版面、表格、公式、阅读顺序等核心能力，同时提供 pipeline、hybrid、VLM 等解析后端。官方文档也明确提供了本地使用方式。citeturn305809search9turn305809search3

因此 OmniMD 不应该重新开发：

- OCR
- PDF Layout Parser
- 表格识别
- 公式识别
- 复杂文档阅读顺序
- 扫描 PDF 解析

这些优先交给 MinerU。

---

# 3. 产品价值

OmniMD 的价值不是：

> “我的解析算法比 MinerU 更强。”

而是：

> **“我把专业文档解析能力变成一个普通人愿意每天使用的桌面软件。”**

目标用户：

### A. AI 用户

经常把 PDF、Word、网页、论文交给 ChatGPT / Claude / NotebookLM。

### B. 知识管理用户

使用 Markdown、Obsidian、Logseq 等本地知识库。

### C. 办公用户

需要把 Word / PDF / PPT / Excel 转成 Markdown。

### D. 开发者 / AI Agent 用户

需要 Markdown 文档进入 Git、RAG、Cursor、Claude Code、Agent 工作流。

---

# 4. 产品核心原则

## 4.1 极简

用户不应该理解：

- MinerU
- OCR
- VLM
- Pipeline
- Hybrid
- Model
- Runtime

用户只需要理解：

> **导入 → Markdown → 使用**

## 4.2 本地优先

本地文件默认：

- 在本机处理
- 不上传
- 不需要登录
- 不需要 API Key

## 4.3 免费

基础核心功能完全免费。

## 4.4 Markdown 是最终数据格式

OmniMD 不把用户锁在自己的数据库里。

用户最终得到的是：

```text
document.md
assets/
```

文件属于用户自己。

## 4.5 文件优先

不要设计成必须注册账户、同步云端的 SaaS。

本地文件夹就是知识库。

---

# 5. 产品结构

整体分为 3 层：

## 第一层：Import

> Anything → Markdown

支持：

- 单文件
- 多文件
- 文件夹
- URL
- 剪贴板内容（后续）

## 第二层：Workspace

> Markdown → 阅读 / 编辑 / 搜索 / 收藏 / 管理

## 第三层：Knowledge

> Markdown → 本地知识库

未来可以增加：

- AI 总结
- AI 问答
- RAG
- MCP
- Agent
- ChatGPT / Claude 工作流

第一阶段不要急着做第三层高级 AI。

---

# 6. 核心技术架构

推荐：

```text
                         OmniMD
                            │
                     React / Tauri UI
                            │
                       Task Manager
                            │
                    Conversion Service
                            │
                       Engine Router
                            │
                    ┌───────┴────────┐
                    │                │
               MinerU Engine     Native Engine
                    │                │
                  MinerU        TXT / CSV / etc.
                    │
                    ↓
              Markdown Output
                    ↓
             OmniMD Postprocess
                    ↓
              Asset Management
                    ↓
             Workspace / Storage
                    ↓
             Editor / Preview / Search
```

---

# 7. MinerU 的定位

MinerU 是 OmniMD 的：

> **主要复杂文档解析引擎**

优先用于：

- PDF
- 扫描 PDF
- 图片
- DOCX
- PPTX
- XLSX

MinerU 官方当前文档列出的输入包含 PDF、image、DOCX、PPTX、XLSX，并输出 Markdown / JSON。citeturn305809search9

---

# 8. anydoc 的定位

第一阶段：

> **不要强制集成 anydoc。**

架构层只保留：

```text
DocumentEngine
```

当前只实现：

```text
MinerUEngine
```

未来如果 benchmark 证明 anydoc 对某些轻量格式明显更快、更小，再加入：

```text
AnyDocEngine
```

即：

```text
DocumentEngine
├── MinerUEngine
└── AnyDocEngine（未来）
```

原则：

> 架构预留 ≠ 现在就集成。

不要为了“备用”增加两套解析系统的安装包、依赖、测试和维护成本。

---

# 9. MinerU 接入方式

AI Agent 必须首先评估以下方案：

## 方案 A：独立进程

```text
Tauri
 ↓
MinerU Process
 ↓
Output
```

优点：

- 崩溃隔离
- 依赖隔离
- 版本容易更新

缺点：

- 进程启动
- 打包复杂

## 方案 B：随应用打包 Python Runtime + MinerU

```text
Tauri
 ↓
Bundled Python
 ↓
MinerU
```

优点：

- 贴近官方 Python 生态

缺点：

- 安装包大
- Windows 打包复杂

## 方案 C：其他原生集成

如果当前 MinerU 版本支持合适的原生/服务化方式，可以评估。

最终方案必须通过真实 Windows benchmark 决定。

---

# 10. 第一阶段最重要的技术验证

在正式重构 UI 前，AI Agent 必须先完成一个独立 PoC：

```text
sample.pdf
↓
OmniMD / MinerU Adapter
↓
MinerU
↓
Markdown
```

验证：

- Windows x64
- CPU 环境
- 模型加载
- 首次启动时间
- 后续启动时间
- 转换速度
- 峰值 RAM
- 磁盘空间
- 输出目录
- 进程退出码
- 失败处理
- 取消任务
- 模型缓存

至少使用：

- 10 页 PDF
- 100 页 PDF
- 扫描 PDF
- 复杂表格 PDF
- DOCX
- PPTX
- XLSX
- 图片

再决定最终集成方式。

---

# 11. 解析模式

普通用户不要看到技术名词。

UI 只提供：

```text
解析质量

● 自动
○ 快速
○ 高质量
```

内部可以映射：

```text
自动 → MinerU Auto / 推荐模式
快速 → pipeline
高质量 → hybrid / VLM（根据设备与实际版本）
```

但具体映射必须根据当前 MinerU 版本的实际能力确认。

MinerU 当前官方示例提供 `hybrid-engine`、`pipeline`、`vlm-engine` 等后端，并支持 `parse_method=auto/txt/ocr` 等参数。citeturn305809search3turn305809search4

普通用户不需要知道这些。

---

# 12. 默认解析策略

默认：

> **自动**

系统根据：

- 文件类型
- 是否存在文本层
- 文档复杂度
- 用户机器能力

自动选择合理解析路线。

不要让用户手动选择：

- OCR
- TXT
- VLM
- Hybrid
- Pipeline

目标：

> **程序替用户做技术决策。**

---

# 13. 首页

首页应该彻底从“转换器”升级为“工作台入口”。

推荐文案：

# 把任何文件变成 AI 可用的 Markdown

副标题：

> 本地处理 · 免费 · 无需登录

格式：

> PDF · Word · PowerPoint · Excel · 图片 · EPUB · 网页

---

# 14. 首页核心区域

核心拖放框：

```text
┌─────────────────────────────────────────┐
│                                         │
│       拖入文件或文件夹到这里              │
│                                         │
│       PDF · DOCX · PPTX · XLSX          │
│       图片 · EPUB 等                     │
│                                         │
│             [ 选择文件 ]                 │
│                                         │
├─────────────────────────────────────────┤
│ 文件 / 文件夹                    粘贴 URL │
└─────────────────────────────────────────┘
```

要求：

- 第一眼知道怎么用
- 不堆功能
- 不出现开发术语
- 视觉上留白充足
- 上传入口明显

---

# 15. 首页空状态

没有历史记录时：

不要展示一个巨大的空白“最近转换”。

建议：

```text
最近

还没有转换记录

拖入你的第一个文件开始
```

有历史后自动展示最近任务。

---

# 16. 导航

建议从当前：

```text
首页
转换
批处理
设置
```

简化成：

```text
首页
知识库
历史
设置
```

原因：

> 单个文件、多文件、文件夹，本质都是导入/转换。

用户不应该在操作前先判断：

> “我要点转换还是批处理？”

系统自动识别：

```text
1 文件 → 单任务
多文件 → 批量
文件夹 → 递归
```

---

# 17. 知识库 / Library

这是 OmniMD 与普通转换器最重要的区别。

左侧：

```text
知识库

📁 AI
📁 产品
📁 论文
📁 工作
📁 读书

⭐ 收藏

🕘 最近
```

知识库本质就是本地文件夹。

例如：

```text
D:\Knowledge```

用户拥有真正的文件。

---

# 18. Library 主界面

推荐三栏结构：

```text
┌────────────┬─────────────────────┬────────────────────┐
│            │                     │                    │
│ 文件夹     │ Markdown 文件列表    │ Preview / Editor   │
│            │                     │                    │
│ 📁 AI      │ 📄 ChatGPT.md       │ # ChatGPT          │
│ 📁 产品    │ 📄 Claude.md        │                    │
│ 📁 论文    │ 📄 Agent.md        │ ## Features        │
│            │                     │                    │
│ ⭐ 收藏    │ 📄 MCP.md           │ 正文……             │
│ 🕘 最近    │                     │                    │
│            │                     │                    │
└────────────┴─────────────────────┴────────────────────┘
```

三栏可以允许收起：

- 左栏可收起
- 右栏可收起

不同屏幕尺寸要适配。

---

# 19. Markdown 编辑器

不要做成 VS Code。

目标：

> **Typora / Notion 风格的 Markdown 阅读编辑体验，但文件始终是真正的 `.md`。**

提供：

```text
编辑
预览
原始
```

## 编辑

所见即所得。

## 预览

阅读模式。

## 原始

真正 Markdown：

```markdown
# Title

## Section
```

用户可以自由切换。

---

# 20. Markdown 文件列表

中间列表应该更像现代桌面软件，而不是文件资源管理器。

示例：

```text
📄 ChatGPT.md
    今天 20:31 · 12 KB

📄 Claude.md
    昨天 18:42 · 8 KB

📄 Agent.md
    2026/08/12 · 22 KB
```

显示：

- 文件名
- 修改时间
- 文件大小

可选：

- 文件类型
- 收藏状态
- 来源

不要显示过多技术参数。

---

# 21. 全局搜索

必须成为核心功能之一。

顶部搜索：

```text
🔍 搜索 Markdown
```

搜索：

- 文件名
- Markdown 正文
- 标题
- tags
- metadata

例如：

```text
MinerU OCR
```

结果：

```text
📄 MinerU.md
📄 OCR.md
📄 文档解析.md
```

点击结果直接打开对应内容。

后续可以支持全文搜索索引。

---

# 22. 搜索体验

要求：

- 快速
- 本地
- 不依赖服务器
- 支持中文
- 支持英文
- 支持模糊搜索

可采用：

- SQLite FTS
- Tantivy
- 其他本地全文索引

具体技术由 AI Agent 根据当前 Rust/Tauri 架构评估。

---

# 23. 收藏

每个 Markdown：

```text
☆
```

点击：

```text
★
```

左侧：

```text
⭐ 收藏
```

用于快速访问重要文档。

---

# 24. 最近使用

显示：

```text
最近

📄 OpenAI Agent Guide
📄 MinerU
📄 Claude Code
📄 Agent Architecture
```

点击直接打开。

保存：

- 文件路径
- 最后打开时间

不要复制 Markdown 内容到数据库。

---

# 25. 导入工作流

OmniMD 最核心的 UX：

```text
拖入文件
↓
自动解析
↓
自动进入当前知识库
↓
Markdown
↓
立即打开
```

不要强迫用户：

```text
转换
→ 保存
→ 找到文件
→ 再导入知识库
```

转换与管理要成为一个完整流程。

---

# 26. 导入方式

首页或 Library 支持：

```text
📄 文件
📁 文件夹
🔗 URL
```

未来：

```text
📋 剪贴板
```

---

# 27. 文件夹导入

用户拖入：

```text
Documents/
```

程序：

```text
递归扫描
↓
解析支持文件
↓
保持目录结构
↓
输出 Markdown
```

例如：

```text
Documents/
├── AI/
│   ├── paper.pdf
│   └── notes.docx
└── Product/
    └── plan.pptx
```

输出：

```text
Knowledge/
├── AI/
│   ├── paper.md
│   └── notes.md
└── Product/
    └── plan.md
```

---

# 28. 输出结构

统一：

```text
document/
├── document.md
└── assets/
    ├── image-001.png
    ├── image-002.png
    └── ...
```

要求：

- Markdown 与 assets 同级目录
- 所有图片用相对路径
- 批量转换不覆盖其他文档
- 文件名安全
- 防止目录穿越
- 支持同名文件自动处理

---

# 29. Assets 管理

右侧或详情面板允许查看：

```text
Assets

🖼 image-001.png
🖼 image-002.png
📊 table-001
```

点击图片：

> 预览

未来支持：

> 在 Markdown 中定位

注意：

> 不要让用户必须理解 assets 才能正常使用 OmniMD。

---

# 30. 文档详情

右上角：

```text
ⓘ
```

点击：

```text
Document Info

标题：
OpenAI Agent Guide

来源：
openai.com

创建：
2026-08-16

修改：
2026-08-16

字数：
12,483

图片：
18

表格：
6

来源类型：
PDF

解析方式：
Auto
```

技术信息默认折叠。

---

# 31. 转换结果页

保留当前双栏设计，但更加产品化：

```text
← 返回

📄 report.pdf

✓ 转换完成 · 8.2 秒

[ 编辑 ] [ 预览 ] [ 原始 ]

                     [ 复制 Markdown ]
                     [ 保存 ]
                     [ 打开文件夹 ]
```

常用动作必须足够明显。

---

# 32. 转换任务状态

对于长任务：

```text
正在处理：report.pdf

✓ 准备文件
✓ 解析文档
● 处理第 24 / 32 页
○ 生成 Markdown
```

显示：

- 当前阶段
- 当前页
- 总页数
- 总进度

不要只显示百分比。

---

# 33. 批量任务界面

批量任务不是一级导航，但必须有专门任务页/弹层。

顶部：

```text
全部 12
处理中 2
已完成 8
失败 2
```

单个任务：

```text
┌──────────────────────────────────┐
│ report.pdf                       │
│ 正在解析第 24 / 32 页             │
│ ███████████████░░░ 82%           │
└──────────────────────────────────┘
```

支持：

- 暂停
- 继续
- 取消
- 重试
- 重试失败项

---

# 34. 失败处理

必须显示真实原因。

例如：

```text
转换失败

文件：example.pdf

原因：
解析引擎无法读取该文件

建议：
- 检查文件是否损坏
- 尝试重新导入
- 尝试高质量模式
```

批量：

```text
100 个文件

✓ 96 成功
⚠ 2 有警告
✕ 2 失败
```

支持：

> 重试失败项

---

# 35. Windows 右键菜单

这是 MVP 之外最重要的产品差异化功能之一。

右键：

```text
使用 OmniMD 转换为 Markdown
```

文件夹：

```text
使用 OmniMD 批量转换为 Markdown
```

要求：

- 支持单文件
- 支持多文件
- 支持文件夹
- 支持后台任务
- 支持打开主窗口查看进度

目标：

> 用户看到 PDF 时，无需主动打开 OmniMD。

---

# 36. 工作区 / Workspace

用户可以创建：

```text
我的 AI 知识库
```

实际上对应一个本地目录：

```text
D:\Knowledge\AI```

OmniMD 只负责：

- 展示
- 搜索
- 编辑
- 导入
- 管理

不把用户数据锁在特殊数据库里。

---

# 37. 多工作区

第一阶段可支持简单切换：

```text
工作区

AI
工作
论文
个人
```

每个工作区对应本地目录。

未来才考虑：

- 最近工作区
- 工作区颜色
- 快捷键

---

# 38. 本地存储原则

Markdown 本体：

> 永远保存在用户自己的文件系统。

OmniMD 的数据库只记录：

- 最近文件
- 收藏
- 最近打开时间
- 搜索索引
- 工作区配置

数据库损坏不能影响 Markdown 正文。

---

# 39. 数据库建议

如果需要本地索引：

> SQLite

可用于：

- History
- Favorites
- Workspace metadata
- Full-text search index

但不要把 Markdown 正文作为唯一数据源存储在 SQLite。

源文件始终是 `.md`。

---

# 40. 编辑器能力

MVP：

- 编辑
- 保存
- 撤销
- 重做
- 自动保存（可选）
- 快捷键
- 标题
- 粗体
- 斜体
- 列表
- 链接
- 代码块

暂时不做：

- 多人协作
- 实时云同步
- 评论
- 版本管理系统

---

# 41. Markdown 渲染

Preview 必须正确处理：

- H1-H6
- 段落
- 列表
- 表格
- 引用
- 图片
- 链接
- 代码块
- LaTeX / 公式
- 行内代码

MinerU 输出中的复杂表格、公式等必须在 Preview 里尽可能保真。

---

# 42. 主题设计

继续保留：

- Light
- Dark
- System

视觉方向：

> Linear + Finder + Typora

关键词：

- 干净
- 精致
- 克制
- 现代
- 少按钮
- 大留白
- 高信息密度但不拥挤

不要变成：

- 极客终端风
- AI 炫技风
- 彩色卡片堆砌
- 复杂 SaaS Dashboard

---

# 43. 快捷键

至少：

```text
Ctrl/Cmd + O
打开文件

Ctrl/Cmd + Shift + O
打开文件夹

Ctrl/Cmd + S
保存

Ctrl/Cmd + F
文档内搜索

Ctrl/Cmd + Shift + F
全局搜索

Ctrl/Cmd + P
快速打开文档

Ctrl/Cmd + N
新建 Markdown
```

具体跨平台行为由 AI Agent 适配。

---

# 44. 新建 Markdown

用户不仅可以导入文件，也可以：

> 新建 Markdown

直接进入编辑器。

例如：

```text
Untitled.md
```

输入内容。

保存到当前工作区。

这样 OmniMD 才真正成为：

> Markdown 工作台

而不只是转换器。

---

# 45. URL 导入

首页：

> 粘贴 URL

流程：

```text
URL
↓
网页正文提取
↓
Markdown
↓
加入当前工作区
```

必须提示：

> URL 导入需要访问网络；本地文件默认不会上传。

---

# 46. AI 功能

第一阶段不要做 AI 聊天。

未来可增加：

- 摘要
- 自动标签
- 自动标题
- Frontmatter
- 文档问答
- 选中文本改写

原则：

> AI 是增强能力，不是基础能力。

基础：

> 文件 → Markdown

无需联网。

---

# 47. Obsidian 支持

P1/P2。

输出：

```text
note/
├── note.md
└── assets/
```

可选 Frontmatter：

```yaml
---
title: Example
source: https://example.com
created: 2026-08-16
tags:
  - imported
---
```

本质仍然是：

> 标准 Markdown + assets

不要引入不可迁移的专有格式。

---

# 48. CLI

P2：

```bash
omnimd report.pdf
```

```bash
omnimd ./documents --recursive
```

CLI 必须复用：

```text
Conversion Service
```

不要实现第二套解析逻辑。

---

# 49. Folder Watch

P2。

用户指定：

```text
Knowledge/
```

监控：

```text
new-document.pdf
```

自动：

```text
new-document.md
```

必须：

- 可关闭
- 支持 debounce
- 防止循环
- 修改 Markdown 不触发反向转换

---

# 50. 搜索实现

第一版要求：

> 本地全文搜索。

技术可以选择：

- SQLite FTS5
- Tantivy

优先考虑：

> 对当前 Tauri/Rust 项目维护成本最低、性能足够的方案。

搜索应支持：

- 文件名
- 标题
- 正文
- tags

---

# 51. 性能目标

重点不是“解析引擎 benchmark 第一”，而是：

> **整个桌面应用使用起来舒服。**

必须关注：

### 启动

- 冷启动
- 热启动

### 转换

- 首次模型加载
- 后续转换
- 批量转换

### Workspace

- 打开 100 个 Markdown
- 打开 1000 个 Markdown
- 全文搜索

### Preview

- 大 Markdown
- 大量图片
- 大表格

### 资源

- CPU
- RAM
- 磁盘
- 模型缓存

---

# 52. 安装包策略

MinerU 可能带来较大的：

- Runtime
- 模型
- 第三方依赖

因此必须先 benchmark。

比较：

### 完整离线包

优点：

- 安装后立即使用
- 完全离线

缺点：

- 安装包大

### 基础包 + 模型下载

优点：

- 初始安装包小

缺点：

- 首次使用需要联网

最终根据真实用户体验决定。

---

# 53. 模型管理

如果需要下载模型：

设置页：

```text
模型

基础模型      1.8 GB
高质量模型   3.2 GB

[ 下载 ]
[ 更新 ]
[ 清理缓存 ]
```

要求：

- 显示大小
- 下载进度
- 可重试
- 校验完整性
- 不重复下载

---

# 54. MinerU 许可证与合规

当前 MinerU 官方 `LICENSE.md` 明确：

- 基于 Apache License 2.0；
- 另有附加条款；
- 满足月活跃用户超过 1 亿或月总收入超过 2000 万美元任一门槛时，需要取得单独商业许可；
- 如果基于 MinerU 向第三方提供在线服务，需要在产品/服务界面或公开文档中显著标明使用 MinerU；
- 未履行适用条件时，许可可能自动终止。citeturn305809search0

本项目第一阶段：

> **仅做本地桌面客户端，不提供基于 MinerU 的在线 SaaS。**

但 AI Agent 必须：

1. 保留 MinerU License；
2. 保留 Apache 相关 NOTICE；
3. 建立第三方依赖清单；
4. 单独核查模型权重许可证；
5. 建立 `THIRD_PARTY_LICENSES`；
6. 发布前进行许可证复核。

注意：

> **不能把“MinerU 使用 Apache 2.0 + 附加条款”理解为 MinerU 全部依赖和模型都自动是 Apache 2.0。**

---

# 55. OSS 合规目录

项目增加：

```text
licenses/
  MINERU-LICENSE.md
  THIRD-PARTY-NOTICES.md
  MODEL-LICENSES.md
```

每项记录：

- 名称
- 版本
- 来源
- License
- 是否随安装包发布
- 是否修改
- 备注

---

# 56. 本地隐私

必须明确：

> 本地文件默认在本机处理。

不得默认：

- 上传文件
- 上传 Markdown
- 上传图片
- 上传 OCR 内容
- 上传路径
- 上传文件名

URL 和未来用户主动启用的 AI 功能例外。

---

# 57. 错误与日志

日志记录：

- task id
- file type
- engine
- start/end time
- error
- exit code
- runtime state

禁止默认记录：

- Markdown 全文
- OCR 全文
- 用户文件内容

日志目录应该可以打开。

普通用户不要看到大量 debug 信息。

---

# 58. MinerU 崩溃隔离

如果可行，优先独立进程：

```text
OmniMD
   │
   └── MinerU Process
           ↓
         crash
           ↓
      Task Manager
           ↓
       捕获错误
           ↓
        UI 提示
```

要求：

> MinerU 崩溃不能让 OmniMD 主界面一起崩溃。

---

# 59. 取消任务

用户点击：

> 取消

应该：

```text
取消任务
↓
终止 MinerU 子进程 / 停止运行
↓
删除临时文件
↓
释放资源
↓
状态 = cancelled
```

不能出现：

> UI 已显示取消，但 MinerU 仍继续运行。

---

# 60. Fallback

第一阶段不强制 anydoc fallback。

未来如果加入：

```text
MinerU
 ↓
failed
 ↓
AnyDoc（如果适用）
```

必须明确提示：

> ⚠ 主解析方式失败，已使用备用解析方式。

不要静默改变输出来源。

---

# 61. 第一阶段 MVP

必须：

- MinerU 集成
- PDF
- DOCX
- PPTX
- XLSX
- 图片
- 单文件
- 多文件
- 文件夹
- Markdown 输出
- Preview
- 编辑
- 本地工作区
- 最近
- 收藏
- 基础搜索
- 历史
- 错误处理
- Windows 右键菜单
- 本地运行
- License / Third-party Notice

---

# 62. 第一阶段暂不做

暂时不做：

- 云同步
- 账号
- 在线存储
- 团队协作
- 在线 SaaS
- AI 聊天
- RAG
- MCP
- CLI
- Folder Watch
- 复杂 Obsidian 双向同步
- Notion 同步
- 社交功能

---

# 63. 开发顺序

## Phase 1：完整分析现有项目

AI Agent 必须先读取：

- Tauri
- Rust
- React / TypeScript
- anydoc
- Task Manager
- Markdown Pipeline
- Preview
- History
- Settings
- Build / Packaging

输出：

1. 当前架构图
2. 技术债
3. MinerU 集成方案
4. 具体改动文件
5. 新依赖
6. 安装包风险
7. Windows 风险
8. License 风险

不要直接大改。

---

## Phase 2：MinerU PoC

完成：

```text
PDF
↓
MinerU
↓
Markdown
```

测：

- 启动
- 内存
- CPU
- 输出
- 失败
- 取消
- 模型

---

## Phase 3：Engine Adapter

实现：

```text
DocumentEngine
└── MinerUEngine
```

为未来 anydoc 留出接口。

---

## Phase 4：Task Manager

接入：

- queue
- progress
- cancel
- retry
- failure

---

## Phase 5：Workspace

实现：

- Library
- Folder Tree
- Markdown List
- Preview
- Editor
- History
- Favorites

---

## Phase 6：搜索

增加：

- 文件名搜索
- 全文搜索

---

## Phase 7：Windows Shell Integration

加入：

> 使用 OmniMD 转换为 Markdown

---

## Phase 8：性能与安装包

测：

- 包大小
- 模型大小
- 启动时间
- 100 页 PDF
- 批量 100 文件
- 1000 Markdown Library

---

## Phase 9：OSS Compliance

完善：

```text
licenses/
```

---

# 64. 验收标准

## 核心体验

- [ ] 用户第一次启动后能在 30 秒内完成第一次转换
- [ ] 拖入文件即可开始
- [ ] 不需要理解 MinerU
- [ ] 不需要 Python
- [ ] 不需要命令行
- [ ] 不需要账号
- [ ] 不需要 API Key

## 转换

- [ ] PDF
- [ ] DOCX
- [ ] PPTX
- [ ] XLSX
- [ ] 图片
- [ ] 文件夹
- [ ] 批量
- [ ] Markdown
- [ ] Assets

## Workspace

- [ ] 新建 Markdown
- [ ] 编辑
- [ ] Preview
- [ ] 原始 Markdown
- [ ] 搜索
- [ ] 收藏
- [ ] 最近
- [ ] 文件夹
- [ ] 工作区

## Windows

- [ ] 右键菜单
- [ ] 多文件
- [ ] 文件夹
- [ ] 后台处理

## 稳定性

- [ ] MinerU 崩溃不导致主程序崩溃
- [ ] 取消任务真实停止
- [ ] 大文件不明显卡死
- [ ] 长任务有进度
- [ ] 模型不会重复加载

## 合规

- [ ] MinerU License
- [ ] NOTICE
- [ ] Third-party License
- [ ] Model License

---

# 65. 明确禁止

AI Agent 不得：

1. 重新开发 OCR。
2. 重新开发 PDF Layout Parser。
3. 同时集成 MinerU + anydoc，除非 benchmark 明确证明必要。
4. 直接把 MinerU 代码散落到 React。
5. 让 UI 直接依赖 MinerU 内部 API。
6. 默认把所有模型都塞进安装包而不评估体积。
7. 要求用户安装 Python。
8. 让 MinerU 崩溃导致主程序崩溃。
9. 把 MinerU 的所有工程术语暴露给普通用户。
10. 为了“功能丰富”把首页做成复杂 Dashboard。
11. 把 Markdown 作为专有数据库格式锁死。
12. 把第三方依赖许可证当成与 MinerU 主许可证相同。
13. 在未重新审查许可证的情况下做在线 MinerU SaaS。
14. 一开始就堆 AI、RAG、MCP 等高级功能。

---

# 66. 产品视觉方向

关键词：

> 精致、安静、现代、克制、本地、专业。

参考气质：

- Linear 的精致
- Finder 的文件管理
- Typora 的 Markdown 阅读
- 少量 Obsidian 的知识库体验

不做：

- 复杂 SaaS Dashboard
- 大量彩色卡片
- AI 炫技动画
- 极客终端视觉
- 满屏按钮

---

# 67. 最终产品形态

最终希望用户看到：

```text
┌───────────────────────────────────────────────────────────┐
│ OmniMD                              🔍 搜索      ⚙       │
├────────────┬────────────────────────┬─────────────────────┤
│            │                        │                     │
│ 📁 AI      │ 📄 ChatGPT.md         │ # ChatGPT           │
│ 📁 产品    │ 📄 Claude.md          │                     │
│ 📁 论文    │ 📄 Agent.md           │ ## Overview         │
│            │                        │                     │
│ ⭐ 收藏    │ 📄 MCP.md             │ 正文……              │
│ 🕘 最近    │                        │                     │
│            │                        │                     │
│ ＋ 新建    │                        │                     │
│            │                        │                     │
└────────────┴────────────────────────┴─────────────────────┘
```

用户拖入：

```text
paper.pdf
```

后：

```text
paper.pdf
↓
MinerU
↓
paper.md
↓
自动加入当前 Workspace
↓
立即打开
```

用户未来可以：

```text
搜索
编辑
阅读
收藏
整理
导入更多文件
```

---

# 68. 最终产品愿景

OmniMD 的长期方向：

```text
                       OmniMD
                          │
            ┌─────────────┴─────────────┐
            │                           │
         Import                      Workspace
            │                           │
    ┌───────┼────────┐        ┌─────────┼─────────┐
    │       │        │        │         │         │
   PDF    Word      URL     Edit      Search    Preview
    │       │        │        │         │         │
    └───────┴────────┘        └─────────┴─────────┘
            │                           │
            └────────── Markdown ───────┘
                          │
                    Local Knowledge
                          │
                  Future AI / RAG / MCP
```

最终：

> **MinerU 负责“把复杂内容变成高质量 Markdown”。**
>
> **OmniMD 负责“让用户真正愿意使用这些 Markdown”。**

OmniMD 的竞争力不是解析引擎，而是：

> **更简单 + 更漂亮 + 更本地 + 更自由 + 更像一个真正的桌面工具。**

---

# 69. AI Agent 最终执行指令

你现在负责在现有 OmniMD 项目上，把产品逐步演进为：

> **基于 MinerU 文档解析能力的本地 Markdown 工作台。**

必须遵守：

1. 先分析现有项目，再修改。
2. 先做 MinerU PoC，再决定最终集成方式。
3. 当前版本只实现 MinerU Engine。
4. anydoc 仅保留架构扩展位，不默认集成。
5. UI 与 MinerU 完全解耦。
6. MinerU 优先独立进程或可隔离方式运行。
7. 优先优化 Windows 安装、模型和 runtime。
8. 转换不是唯一功能，Workspace 是核心长期价值。
9. Markdown 必须是真实 `.md` 文件。
10. 本地文件默认不上传。
11. 不做账号和云同步。
12. 首页面向普通用户，不暴露底层技术。
13. 第一阶段先做 Import + Workspace + Preview + Search + History。
14. Windows 右键菜单是重要差异化能力。
15. 不要重复开发 OCR / Layout / Table Parser。
16. 所有第三方依赖和模型必须做许可证清单。
17. 所有技术决策优先通过真实 benchmark，而不是凭感觉。
18. 不要因为“以后可能需要”提前塞入大量复杂依赖。

最终验收不是：

> “OmniMD 的解析能力是否超过 MinerU？”

而是：

> **“普通用户是否更愿意使用 OmniMD 完成 文件 → Markdown → 阅读/编辑/管理 这一整套流程。”**
