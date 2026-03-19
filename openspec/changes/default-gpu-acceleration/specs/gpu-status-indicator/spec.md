## ADDED Requirements

### Requirement: GPU status data binding

AppState 全局单例 SHALL 通过 `gpu-available: bool`（默认 `false`）和 `gpu-name: string`（默认 `""`）两个属性传递 GPU 状态。Rust 侧 MUST 在 `setup_callbacks` 中、`ui.run()` 之前写入这两个属性。`set_gpu_name()` 调用 MUST NOT 被 `#[cfg(feature = "gpu")]` 条件编译包裹；`GpuAccelerator` 在 `gpu` feature 未启用时 MUST 提供 `name()` 方法返回 `""`。

#### Scenario: Properties ready before event loop

- **WHEN** 应用完成 `App::new()` 并进入 `setup_callbacks`
- **THEN** `state.set_gpu_available(...)` 和 `state.set_gpu_name(...)` MUST 在 `ui.run()` 之前完成调用，用户看到的第一帧中 GPU 状态文本已反映最终状态

#### Scenario: No cfg guard on gpu_name setter

- **WHEN** 代码调用 `state.set_gpu_name(gpu.name().into())`
- **THEN** 该调用 MUST 不被任何 `#[cfg]` 条件编译包裹，无论 `gpu` feature 是否启用均可编译通过

### Requirement: GPU available display

当 GPU 成功初始化时，Footer GPU 状态文本 SHALL 显示 `"GPU: {adapter_name}"`，其中 `{adapter_name}` 为 wgpu 返回的 `adapter.get_info().name` 原始字符串。

#### Scenario: Discrete GPU available

- **WHEN** `gpu-available` 为 `true` 且 `gpu-name` 为 `"NVIDIA GeForce RTX 4090"`
- **THEN** Footer GPU 状态文本 MUST 显示 `"GPU: NVIDIA GeForce RTX 4090"`

#### Scenario: Integrated GPU available

- **WHEN** `gpu-available` 为 `true` 且 `gpu-name` 为 `"Intel(R) UHD Graphics 770"`
- **THEN** Footer GPU 状态文本 MUST 显示 `"GPU: Intel(R) UHD Graphics 770"`

#### Scenario: Adapter name with special characters

- **WHEN** `gpu-available` 为 `true` 且 `gpu-name` 包含括号、注册商标符号或非 ASCII 字符
- **THEN** Footer GPU 状态文本 MUST 原样展示适配器名称，不做截断或转义

### Requirement: GPU unavailable runtime display

当 `gpu` feature 已编译但 GPU 运行时初始化失败时，Footer GPU 状态文本 SHALL 显示 `"GPU: 不可用 (CPU 模式)"`。MUST NOT 包含 `"需 --features gpu 编译"` 或任何编译指引文案。

#### Scenario: No adapter found

- **WHEN** `gpu` feature 已编译且 wgpu `request_adapter` 返回 `None`
- **THEN** `gpu-available` MUST 为 `false`，Footer MUST 显示 `"GPU: 不可用 (CPU 模式)"`

#### Scenario: Device creation failure

- **WHEN** `gpu` feature 已编译且 `request_adapter` 成功但 `request_device` 返回错误
- **THEN** `gpu-available` MUST 为 `false`，Footer MUST 显示 `"GPU: 不可用 (CPU 模式)"`

#### Scenario: Initialization panic caught

- **WHEN** GPU 初始化过程中驱动层触发 Rust panic（由 `catch_unwind` 捕获）
- **THEN** `gpu-available` MUST 为 `false`，Footer MUST 显示 `"GPU: 不可用 (CPU 模式)"`，panic MUST 被记录到 tracing 日志（`error` 级别），应用 MUST NOT 退出

### Requirement: GPU timeout fallback display

当 GPU 初始化超过 3 秒超时阈值时，系统 SHALL 自动降级到 CPU 模式，返回 `GpuAccelerator::unavailable()` 实例。

#### Scenario: Initialization timeout triggers fallback

- **WHEN** `GpuAccelerator::try_new_sync()` 中的子线程在 3 秒内未返回结果
- **THEN** 主线程 MUST 停止等待并返回 `GpuAccelerator::unavailable()`
- **THEN** `gpu-available` MUST 为 `false`，Footer MUST 显示 `"GPU: 不可用 (CPU 模式)"`

#### Scenario: Timeout does not block application startup

- **WHEN** GPU 初始化触发超时
- **THEN** 从应用启动到 UI 窗口可见的总耗时 MUST NOT 因 GPU 超时而超过 5 秒

#### Scenario: Timeout logged as warning

- **WHEN** GPU 初始化超时发生
- **THEN** MUST 通过 tracing 记录一条 `warn` 级别日志，内容包含 "timeout" 关键词和超时阈值（3 秒）

#### Scenario: Late thread result discarded after timeout

- **WHEN** GPU 初始化因超时返回 `unavailable()` 后，子线程最终完成初始化
- **THEN** 该延迟结果 MUST 被丢弃，`gpu-available` MUST 维持 `false`

### Requirement: GPU feature disabled display

当二进制通过 `--no-default-features` 编译（未包含 `gpu` feature）时，Footer GPU 状态文本 SHALL 显示 `"GPU: 不可用 (CPU 模式)"`。

#### Scenario: Pure CPU build

- **WHEN** 应用通过 `cargo build --no-default-features` 编译
- **THEN** `GpuAccelerator::try_new_sync()` MUST 返回 `available: false` 实例
- **THEN** `GpuAccelerator::name()` MUST 返回 `""`
- **THEN** Footer MUST 显示 `"GPU: 不可用 (CPU 模式)"`

#### Scenario: No wgpu linkage in CPU build

- **WHEN** 应用通过 `--no-default-features` 编译
- **THEN** 编译产物 MUST NOT 链接 wgpu 相关库

### Requirement: GPU status text consistency

所有 GPU 状态场景的文本 SHALL 遵循统一格式：以 `"GPU: "` 前缀开头（含一个半角空格），GPU 可用时后接适配器名称原文，GPU 不可用时（无论原因）后接 `"不可用 (CPU 模式)"`。整个系统只存在这两种文案分支。

#### Scenario: Exhaustive text variants

- **WHEN** 枚举所有可能的 GPU 状态：feature 编译+成功、feature 编译+失败、feature 编译+超时、feature 未编译
- **THEN** 第一种显示 `"GPU: {adapter_name}"`，后三种 MUST 全部显示 `"GPU: 不可用 (CPU 模式)"`，不存在第三种文案模板

### Requirement: GPU status indicator layout

GPU 状态指示器 SHALL 位于 Footer 水平布局左侧，字体大小为 `Typography.font-size-xs`，字体颜色为 `Palette.text-muted`，垂直居中对齐。Footer SHOULD 为单行固定高度区域（当前设计为 28px），背景色 MUST 为 `Palette.bg-secondary`。

#### Scenario: Footer layout structure

- **WHEN** 主窗口渲染 Footer 区域
- **THEN** 左侧第一个元素 MUST 为 GPU 状态文本，右侧最后一个元素为版本号文本，两者之间由弹性空白分隔

### Requirement: GPU status i18n support

GPU 状态文本中 `"不可用 (CPU 模式)"` 部分 SHALL 通过 Slint `@tr()` 宏标记为可翻译字符串。`"GPU: "` 前缀作为技术术语 MAY 保持不翻译。GPU 可用时的适配器名称 MUST NOT 经过翻译处理。

#### Scenario: Unavailable text translatable

- **WHEN** GPU 不可用且当前语言为英文
- **THEN** 状态文本 MUST 显示对应翻译后的文案（如 `"GPU: Unavailable (CPU mode)"`）

#### Scenario: Adapter name not translated

- **WHEN** GPU 可用且当前语言为中文
- **THEN** 适配器名称 MUST 保持 wgpu 返回的原始英文字符串

### Requirement: GPU state immutability after init

GPU 状态属性 SHALL 在 `setup_callbacks` 完成后保持不变直到应用退出。即使 GPU 在运行时发生 device lost 事件，`gpu-available` 和 `gpu-name` 也 MUST 维持初始化时的值。

> **Note**: 此不可变约束在后续 P2.1（运行时 GPU 故障恢复：device lost callback + AtomicBool）实施时将被修改为允许 `gpu-available` 在 device lost 事件后从 `true` 变为 `false`。

#### Scenario: Properties frozen after event loop starts

- **WHEN** `ui.run()` 启动事件循环后
- **THEN** `gpu-available` 和 `gpu-name` 的值 MUST NOT 被任何代码路径修改
