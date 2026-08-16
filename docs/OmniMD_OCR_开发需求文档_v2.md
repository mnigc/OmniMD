# OmniMD OCR 能力开发需求文档 v2.0

## 1. 项目背景

OmniMD 是基于 Tauri 2.x + React 18 + TypeScript + Tailwind CSS 的本地桌面应用，定位为：

> Anything → AI-ready Markdown

当前已经支持 PDF、DOCX、PPTX、XLSX、EPUB、CSV、TXT、HTML、RTF、ODT 等格式转换，以及单文件、批量处理、Markdown/Preview、保存、历史记录等能力。

本次新增 OCR，目标是：

> 扫描 PDF / 图片型 PDF / 图片 → 高质量、结构化 Markdown

OCR 必须成为现有转换 Pipeline 的一个模块，而不是一套独立流程。

---

# 2. OCR 核心选型：只运行一套 OCR

## 2.1 最终方案

第一阶段只使用：

> **PP-OCRv6**

作为 OCR 模型体系。

本地推理：

> **ONNX Runtime**

工程适配：

> **优先评估 RapidOCR 作为 PP-OCRv6 的 ONNX 本地调用层**

重要：

**不要把 PaddleOCR 和 RapidOCR 当成两个独立 OCR 引擎同时集成。**

正确理解：

```text
PP-OCRv6
   ↓
ONNX Runtime
   ↓
RapidOCR（可选适配层）
   ↓
OmniMD OCR Service
```

如果 RapidOCR 与当前 Tauri/Rust/Windows 架构不匹配，直接采用 PP-OCRv6 + ONNX Runtime 原生集成即可。

最终目标只有：

> **应用内部只有一套实际运行的 OCR 模型：PP-OCRv6。**

---

# 3. 为什么采用这个路线

OmniMD 的核心特点：

- 免费
- 本地运行
- 隐私优先
- Windows 桌面应用
- 不依赖云端 API

OCR 必须：

- 本地运行
- 不需要 API Key
- 默认不上传文件
- 支持 CPU
- Windows 稳定部署
- 可随软件发布
- 支持模型缓存
- 不影响普通 PDF 转换

---

# 4. 必须实现的场景

## 4.1 普通 PDF

有有效文本层：

```text
PDF
↓
检测文本层
↓
正常解析
↓
不启动 OCR
```

## 4.2 扫描 PDF

无有效文本层：

```text
PDF
↓
检测无有效文本
↓
OCR
↓
Markdown
```

## 4.3 混合 PDF

不同页面分别处理：

```text
Page 1 → 原生文本
Page 2 → OCR
Page 3 → 原生文本
Page 4 → OCR
```

最后统一合并。

## 4.4 图片

至少支持：

- PNG
- JPG / JPEG

其他格式视现有依赖情况决定。

---

# 5. OCR Pipeline

推荐：

```text
输入文件
 ↓
文件类型检测
 ↓
Document Detector
 ↓
判断文本层
 ├─ 有文本层 → anydoc / 原有解析
 └─ 无文本层 → OCR Service
 ↓
OCR Structured Result
 ↓
Markdown Pipeline
 ├─ Normalize
 ├─ Cleanup
 ├─ Asset Manager
 └─ AI Ready Formatter
 ↓
Markdown + assets/
```

---

# 6. 工程架构

推荐目录：

```text
src/
  conversion/
    conversion_service
    document_detector
    markdown_pipeline/
      normalize
      cleanup
      asset_manager
      ai_ready_formatter

  ocr/
    ocr_service
    ocr_engine
    ocr_config
    ocr_result
    ocr_worker
    pdf_renderer
    layout_reconstructor
```

要求：

- OCR 与 UI 解耦
- UI 不直接调用 OCR 模型
- OCR 不直接生成最终 Markdown
- OCR 结果必须先进入结构化中间层

调用关系：

```text
UI
 ↓
Task Manager
 ↓
Conversion Service
 ↓
OCR Service
 ↓
OCR Engine
```

---

# 7. OCR Service 接口

建议统一抽象：

```text
OCRService

initialize()
is_available()
recognize_image()
recognize_page()
recognize_document()
cancel()
shutdown()
```

配置：

```text
OCRConfig

mode
language
model_path
runtime
```

mode：

```text
auto
off
always
```

不要把 RapidOCR / ONNX Runtime 的具体 API 暴露给 UI。

---

# 8. OCRResult 数据结构

OCR Engine 不应直接生成 Markdown。

统一返回结构化结果，例如：

```json
{
  "page": 1,
  "width": 2480,
  "height": 3508,
  "blocks": [
    {
      "type": "text",
      "text": "示例标题",
      "confidence": 0.98,
      "bbox": [100, 120, 1200, 220],
      "order": 1
    }
  ]
}
```

Block 至少包含：

- type
- text
- confidence
- bbox
- page
- order

预留：

- heading
- paragraph
- list
- table
- image
- formula
- caption
- header
- footer

---

# 9. PDF 文本层检测

OCR 前必须先检测文本层。

目标：

> 尽量避免对已经有文本层的 PDF 做 OCR。

建议检测：

- 字符数量
- 可打印字符比例
- 页面文本覆盖率
- 是否存在有效文本

判断：

### 有效文本

走现有 anydoc。

### 文本为空或明显不足

进入 OCR。

### 混合 PDF

逐页判断。

---

# 10. 扫描 PDF 处理

推荐：

```text
PDF
↓
逐页读取
↓
检测有效文本层
↓
有文本 → native extraction
无文本 → PDF 页面渲染成图片
↓
OCR
↓
Layout / Reading Order
↓
Markdown 中间结构
↓
Markdown
```

要求：

- 按页处理
- 不要一次把整个 PDF 全部渲染到内存
- 页面完成后及时释放资源
- 支持大 PDF
- 支持任务取消

---

# 11. PDF 渲染

OCR 前需要将扫描页面转换成图片。

初始建议：

> 200–300 DPI

最终值通过实际 benchmark 决定。

原则：

- 避免过高 DPI
- 控制内存
- 必要时缩放
- 页面处理后释放中间图片

---

# 12. OCR 语言

第一阶段：

- 简体中文
- 英文

默认：

> 自动

UI：

```text
OCR 语言

● 自动
○ 中文
○ 中文 + 英文
○ 英文
```

后续预留：

- 繁体中文
- 日文
- 韩文
- 其他模型支持语言

不要在第一版宣称所有语言拥有同等质量。

---

# 13. 图片预处理

OCR 前可以使用轻量预处理：

- 自动旋转
- 去噪
- 对比度增强
- 倾斜校正
- 分辨率调整

默认：

```text
输入
↓
轻量预处理
↓
OCR
```

不要默认进行昂贵的重度图像处理。

---

# 14. 自动方向

至少支持：

- 0°
- 90°
- 180°
- 270°

用户不应该必须手动旋转图片或页面。

---

# 15. 阅读顺序

OCR 检测框必须进行阅读顺序重建。

## 单栏

```text
上 → 下
```

## 双栏

```text
左栏上
↓
左栏下
↓
右栏上
↓
右栏下
```

## 标题

标题应出现在对应正文前。

## 表格

表格应作为整体处理。

如果无法可靠判断：

> 宁可保守，不要生成错误的阅读顺序。

---

# 16. 标题识别

尽可能恢复：

```markdown
# H1
## H2
### H3
```

可利用：

- bbox 高度
- 文本长度
- 页面位置
- block 类型
- 前后层级

原则：

> 宁可漏识别标题，也不要把大量普通正文误判为标题。

---

# 17. 段落合并

OCR 常常一行一句。

例如：

```text
这是一个很长的
自然段内容，它
原本在 PDF 中是
连续的一段。
```

应尽可能恢复：

```markdown
这是一个很长的自然段内容，它原本在 PDF 中是连续的一段。
```

但标题、列表、表格不能被错误合并。

---

# 18. 页眉、页脚、页码

如果同样文本：

- 出现在页面顶部/底部
- 跨多页重复

应尽可能识别为：

- header
- footer
- page number

并从正文中清理。

不要误删正文。

---

# 19. 表格识别

目标：

```markdown
| 姓名 | 电话 | 地址 |
|---|---|---|
| 张三 | 123 | 上海 |
| 李四 | 456 | 北京 |
```

流程：

```text
页面
↓
Layout
↓
表格区域
↓
表格结构识别
↓
Markdown Table
```

如果无法可靠恢复结构：

> 退化为普通文本，而不要生成看似正确但实际错误的 Markdown 表格。

结果页可提示：

```text
⚠ 检测到表格，但部分结构可能不完整
```

---

# 20. 图片与 assets

OCR 并不等于把整页扫描底图导出。

不要默认把所有 OCR 页面图片写入 assets。

仅保存真正属于文档内容的图片/插图：

```text
document/
  document.md
  assets/
    image-001.png
    image-002.png
```

Markdown：

```markdown
![图片](assets/image-001.png)
```

必须：

- 使用相对路径
- 防止文件覆盖
- 批量任务互不影响

---

# 21. AI Ready Markdown

OCR 完成后统一进入：

```text
Normalize
↓
Cleanup
↓
Asset Manager
↓
AI Ready Formatter
```

重点：

- 清理页眉页脚
- 清理页码
- 合并分页导致的段落断裂
- 修复明显断行
- 保留标题层级
- 保留列表
- 保留表格
- 保留重要图片
- 减少无意义空行

基础 AI Ready 不能依赖 LLM。

---

# 22. OCR 模式

设置页：

```text
OCR

● 自动
○ 始终关闭
○ 始终开启
```

默认：

> 自动

含义：

### 自动

有文本层：

> 不 OCR

无文本层：

> 自动 OCR

### 关闭

> 永远不 OCR

### 始终开启

> 对支持的页面强制 OCR

---

# 23. UI 状态

OCR 比普通转换慢，必须展示明确进度：

```text
正在转换：合同扫描件.pdf

✓ 检测 PDF
✓ 分析文本层
✓ 发现 18 页扫描内容
● OCR 第 7 / 18 页
○ 生成 Markdown
```

至少显示：

- 当前阶段
- 当前页
- 总页数
- 总体进度

不要只显示一个百分比。

---

# 24. 结果页

完成后显示：

```text
✓ 转换完成

页数：18
OCR 页：18
识别字符：12,431
低置信度区域：3
```

如有质量风险：

```text
⚠ OCR 结果可能存在少量识别错误
```

仍然允许：

- 复制 Markdown
- 保存 Markdown
- 打开输出文件夹
- Preview

---

# 25. OCR 置信度

每个 block 保存 confidence。

UI 不需要显示每个字符。

可汇总：

```text
平均置信度：96.2%
低置信度区域：3
```

如果整体置信度较低：

```text
⚠ 该文档可能存在较多 OCR 识别错误
```

不要阻止导出。

---

# 26. 错误处理

不能只显示：

> OCR Failed

需要明确：

```text
OCR 失败

文件：scan.pdf
页面：8

原因：
无法读取页面图像

建议：
- 检查 PDF 是否损坏
- 尝试重新打开文件
- 尝试其他版本文件
```

局部失败：

```text
20 页
✓ 19 页成功
⚠ 1 页失败
```

允许：

> 重试失败页

不能因为单页失败而丢弃整个文档。

---

# 27. 模型加载

必须 Lazy Load。

普通用户转换 DOCX：

```text
不要加载 OCR 模型
```

第一次需要 OCR：

```text
首次 OCR
↓
加载 PP-OCRv6 模型
↓
缓存
↓
后续复用
```

不要每个文件重复加载。

---

# 28. 模型打包

目标：

> 安装 OmniMD 后即可使用 OCR。

优先评估：

### 方案 A：模型随安装包发布

优点：

- 完全离线
- 首次体验最好

缺点：

- 安装包更大

### 方案 B：首次使用时下载

优点：

- 安装包更小

缺点：

- 首次 OCR 需要联网

如果模型体积在可接受范围内：

> 优先随安装包提供。

否则可以首次下载，但 UI 必须清楚提示用户。

---

# 29. Windows 要求

第一阶段重点：

> Windows x64

必须测试：

- 无 Python
- 无开发环境
- Intel CPU
- AMD CPU
- 较低内存
- 长时间运行
- 多次 OCR

最终用户不应需要手动：

- 安装 Python
- 安装 Paddle
- 安装 ONNX Runtime
- 下载模型
- 配环境变量

---

# 30. 性能

OCR：

- 必须后台执行
- 不得阻塞 UI
- 大 PDF 分页处理
- 图像处理后及时释放
- 模型可复用

第一阶段 OCR 建议：

> 默认单任务或低并发

不要为了追求并发把内存和 CPU 打爆。

---

# 31. 批处理

OCR 必须兼容现有批处理。

例如：

```text
100 个 PDF
↓
自动判断
↓
普通 PDF → anydoc
扫描 PDF → OCR
混合 PDF → 部分 OCR
```

结果：

```text
100 个文件

✓ 92 成功
⚠ 6 有警告
✕ 2 失败
```

支持：

> 重试失败项

---

# 32. 文件夹递归

用户拖入：

```text
Documents/
├── report.pdf
├── scan.pdf
├── meeting.docx
└── 2026/
    ├── old_scan.pdf
    └── contract.pdf
```

程序：

```text
递归扫描
↓
自动检测
↓
普通转换 / OCR 自动选择
```

并保持原始目录结构。

---

# 33. 隐私

必须满足：

> OCR 完全在本机运行。

不得自动：

- 上传文件
- 上传 OCR 文本
- 上传 OCR 图片
- 上传文件路径
- 上传文件名

只有：

- URL 转换
- 用户主动启用的 AI 功能

才可以访问网络。

UI 明确写：

> OCR 在本机运行，文件和识别内容不会上传到 OmniMD 服务器。

---

# 34. 第三方许可证

开发前必须核对：

- PP-OCRv6 模型许可证
- ONNX Runtime 许可证
- RapidOCR 许可证（如使用）
- 所有随安装包分发的第三方组件

正确保留 LICENSE / NOTICE。

---

# 35. OCR 回归测试集

至少准备：

## PDF

- 普通 PDF
- 纯扫描 PDF
- 混合 PDF

## 中文

- 中文报告
- 中文合同
- 中文论文
- 中文表格

## 英文

- 英文报告
- 英文论文

## 排版

- 单栏
- 双栏
- 多栏
- 旋转页面

## 图像质量

- 高清
- 低清
- 模糊
- 倾斜
- 阴影
- 低对比度

## 表格

- 有边框
- 无边框
- 合并单元格

---

# 36. 验收标准

## 功能

- [ ] 普通 PDF 有文本层时默认不 OCR
- [ ] 扫描 PDF 自动 OCR
- [ ] 支持混合 PDF
- [ ] 支持图片 OCR
- [ ] 支持中文/英文
- [ ] 支持自动方向
- [ ] 支持批处理
- [ ] 支持取消
- [ ] OCR 失败有明确错误
- [ ] OCR 进入统一 Markdown Pipeline

## 内容

- [ ] 阅读顺序基本正确
- [ ] 段落尽可能正确合并
- [ ] 标题尽可能保留
- [ ] 列表尽可能保留
- [ ] 表格尽可能转 Markdown
- [ ] 页眉页脚尽可能清理
- [ ] 页码尽可能清理
- [ ] 图片相对路径正确
- [ ] Preview 可正常显示

## 性能

- [ ] OCR 后台执行
- [ ] UI 不冻结
- [ ] 模型 Lazy Load
- [ ] 模型复用
- [ ] 大 PDF 分页处理
- [ ] 不因 OCR 导致普通转换明显变慢

## 隐私

- [ ] OCR 默认完全本地
- [ ] 无云端 OCR API
- [ ] 不上传用户内容
- [ ] UI 明确隐私说明

---

# 37. AI Agent 开发顺序

## Phase 1：先分析项目

先读取：

- Tauri 架构
- Rust
- React / TypeScript
- anydoc 调用方式
- PDF 现有逻辑
- Task Manager
- Conversion Service
- Markdown Pipeline
- Settings

先输出：

1. 架构分析
2. 新增模块
3. 修改模块
4. 文件修改列表
5. 新依赖
6. 风险点

先不要大面积重构。

## Phase 2：OCR 抽象层

实现：

```text
OCRService
OCRConfig
OCRResult
OCRBlock
```

先用 mock 验证 UI / Pipeline。

## Phase 3：接入 PP-OCRv6

实现：

```text
PP-OCRv6
+
ONNX Runtime
```

优先验证：

- Windows x64
- CPU
- 中文
- 英文
- PNG/JPG

如果采用 RapidOCR：

> 只作为 PP-OCRv6 ONNX 本地调用/适配层，不得引入第二套 OCR。

## Phase 4：PDF 文本层检测

实现：

```text
has_valid_text_layer(page)
```

有文本：

> native

无文本：

> OCR

## Phase 5：混合 PDF

支持不同页面走不同路径，然后统一合并。

## Phase 6：Markdown Pipeline

实现：

```text
OCRResult
↓
Markdown intermediate representation
↓
Normalize
↓
Cleanup
↓
Assets
↓
AI Ready
```

## Phase 7：UI

增加：

- OCR 设置
- 自动模式
- 当前阶段
- 页码进度
- warning
- 完成统计

保持现有视觉风格。

## Phase 8：真实回归测试

至少测试 20 个真实文档，并记录：

- 转换耗时
- 内存
- OCR 准确度
- Markdown 结构
- 失败率

---

# 38. 明确禁止

AI Agent 不得：

1. 同时集成 PaddleOCR 与 RapidOCR 两套 OCR 引擎。
2. 引入云端 OCR API。
3. 要求用户安装 Python。
4. 默认上传用户文件。
5. 默认对所有 PDF 强制 OCR。
6. 把所有 OCR 页面底图写入 assets。
7. 一次性把大型 PDF 全部渲染进内存。
8. 为 OCR 大规模重构无关模块。
9. 绕开现有 Markdown Pipeline。
10. 因 OCR 破坏现有普通 PDF/DOCX/PPTX/XLSX 转换。

---

# 39. 最终架构

```text
                    OmniMD
                       │
                Conversion Service
                       │
              ┌────────┴────────┐
              │                 │
        Native Extraction      OCR Service
              │                 │
           anydoc          PP-OCRv6
                                │
                          ONNX Runtime
                                │
                    RapidOCR（可选适配层）
                                │
                         OCR Structured Result
              │                 │
              └────────┬────────┘
                       ↓
                Markdown Pipeline
                       ↓
                Normalize / Cleanup
                       ↓
                  Asset Manager
                       ↓
                 AI Ready Formatter
                       ↓
                Markdown + assets/
```

再次强调：

> RapidOCR 不是第二套 OCR，它只是可选的 PP-OCRv6 ONNX 本地调用/适配方案。

如果直接用 ONNX Runtime 集成更简单稳定，则完全可以不使用 RapidOCR。

---

# 40. 最终用户体验

用户不需要理解 OCR。

只需要：

```text
拖入 PDF
↓
OmniMD 自动检测
↓
普通 PDF → 正常解析
扫描 PDF → 自动 OCR
混合 PDF → 自动分页面处理
↓
统一得到高质量 Markdown
```

目标体验：

> “OmniMD 能把我的 PDF 直接变成 Markdown。”

而不是让用户理解“什么是 OCR、这个 PDF 是不是扫描件”。

---

# 41. 参考项目

PaddleOCR：
https://github.com/PaddlePaddle/PaddleOCR

RapidOCR：
https://github.com/RapidAI/RapidOCR

ONNX Runtime：
https://github.com/microsoft/onnxruntime

PP-StructureV3：
https://github.com/PaddlePaddle/PaddleOCR

开发前再次核对：

- PP-OCRv6 当前模型版本
- 模型许可证
- ONNX 导出方式
- Windows x64 兼容性
- ONNX Runtime 版本
- RapidOCR 当前兼容情况
- 安装包体积
- CPU 性能

---

# 42. 给 AI Agent 的最终执行指令

你负责在已有 OmniMD 项目中增加 OCR。

必须遵守：

1. 先分析现有项目，不要立即改代码。
2. 先输出架构分析和文件修改计划。
3. OCR 必须是独立模块。
4. 只使用一套实际 OCR 模型：PP-OCRv6。
5. 使用 ONNX Runtime 做本地推理。
6. RapidOCR 仅为可选适配层，不是第二套 OCR。
7. 普通 PDF 有有效文本层时不得无意义 OCR。
8. 扫描 PDF 自动 OCR。
9. 支持混合 PDF。
10. OCR 后台运行。
11. 大 PDF 按页处理。
12. 模型 Lazy Load 并复用。
13. OCR 输出必须进入现有 Markdown Pipeline。
14. 不得破坏现有转换功能。
15. 不得使用云端 OCR。
16. 不得要求用户安装 Python。
17. 不得同时塞进两套 OCR。
18. 每个阶段完成后测试。
19. 最终在 Windows x64 做真实文档回归测试。

最终目标：

> 用户拖入普通 PDF、扫描 PDF、混合 PDF 或图片，都能通过同一个转换入口，自动获得高质量、结构化、AI-ready Markdown，并且 OCR 默认完全在本机完成。
