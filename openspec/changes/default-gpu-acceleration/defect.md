# 审查缺陷清单：default-gpu-acceleration

> **状态**: 全部 14 个缺陷 + 4 个不自洽点已修复 ✅

## Critical（阻塞性）

### C1: 任务 3.1/3.2 执行顺序依赖未标注 ✅

**状态**: 已修复
**修复方式**: tasks.md 3.2 已添加 `**[依赖 3.1]**` 标注，明确说明必须在 3.1 完成后执行

### C2: `pad_to_blocks` 零尺寸图像 u32 下溢 ✅

**状态**: 已修复
**修复方式**: tasks.md 新增「## 2.5 GPU 编码器防御性检查」任务组，要求在 `GpuJpegEncoder::encode()` 入口添加零尺寸防御检查。P2.4 描述已更新标注最小防御已在 2.5 完成

---

## Major（需修复后方可实施）

### M1: Spec 缺少 D2（Vulkan feature 修复）的需求覆盖 ✅

**状态**: 已修复
**修复方式**: 新建 `specs/gpu-acceleration-policy/spec.md`，包含「Platform-specific GPU backend selection」Requirement，覆盖 D2 和 D6

### M2: 超时后线程资源泄漏风险未显式记录 ✅

**状态**: 已修复
**修复方式**: design.md D3 新增「超时后线程资源清理」段落，说明 Rust ownership 机制保证资源自动释放

### M3: D4 修改后 `create_jpeg_encoder` 文档注释未要求更新 ✅

**状态**: 已修复
**修复方式**: tasks.md 4.1 已补充要求同步更新 `create_jpeg_encoder` 和 `create_encoder` 的文档注释

### M4: D6 + D2 交互：非 Linux 平台编译冗余 Vulkan 后端 ✅

**状态**: 已修复
**修复方式**: design.md D2 新增「与 D6 的交互说明」段落，解释 trade-off 和后续优化方向

### M5: Spec 缺少 D4（JPEG 编码策略）的覆盖 ✅

**状态**: 已修复
**修复方式**: 新建 `specs/gpu-acceleration-policy/spec.md`，包含「JPEG encoding defaults to mozjpeg」和「GPU acceleration limited to preprocessing」两个 Requirement

### M6: Spec 缺少 D6（平台后端限制）的覆盖 ✅

**状态**: 已修复
**修复方式**: 合并到 `specs/gpu-acceleration-policy/spec.md` 的「Platform-specific GPU backend selection」Requirement 中

---

## Minor（不阻塞但应修复）

### m1: Proposal 声称「新增」AppState 属性，实际已存在 ✅

**状态**: 已修复
**修复方式**: proposal.md 第 9 行改为「利用 `AppState` global 中已有的 `gpu-available` 和 `gpu-name` 属性」

### m2: tasks.md 5.2 翻译 key 描述模糊 ✅

**状态**: 已修复
**修复方式**: tasks.md 5.2 已列出具体 key `gpu_unavailable` 和 en/zh-CN/zh-TW/ja 四种语言翻译值

### m3: Spec immutability 与 P2.1 device lost 计划矛盾 ✅

**状态**: 已修复
**修复方式**: spec.md「GPU state immutability after init」添加 blockquote 注释说明 P2.1 将修改此约束

### m4: D5 scope 描述过于宏大 ✅

**状态**: 已修复
**修复方式**: design.md D5 标题改为「统一 `GpuAccelerator` 公开接口，消除业务代码中的条件编译守卫」

### m5: tasks.md 6.4 CLAUDE.md 更新内容不具体 ✅

**状态**: 已修复
**修复方式**: tasks.md 6.4 已列出具体需更新的 4 个 CLAUDE.md 章节

### m6: Spec 硬编码 28px Footer 高度过于严格 ✅

**状态**: 已修复
**修复方式**: spec.md 将 `Footer 高度 MUST 为 28px` 改为 `Footer SHOULD 为单行固定高度区域（当前设计为 28px）`

---

## 不自洽点

| # | 状态 | 矛盾描述 | 修复方式 |
|---|------|----------|----------|
| I1 | ✅ | Proposal 声称新增实际已存在的属性 | proposal.md 措辞已修正（同 m1） |
| I2 | ✅ | spec immutability 与 P2.1 冲突 | spec 添加注释说明未来修改（同 m3） |
| I3 | ✅ | D5 scope 描述过于宏大 | D5 标题已修正（同 m4） |
| I4 | ✅ | app.rs 注释与 D3 风险不一致 | design.md D3 补充说明 app.rs 注释需同步更新 |
