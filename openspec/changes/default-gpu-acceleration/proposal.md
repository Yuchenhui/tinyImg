## Why

当前 GPU 加速（wgpu compute shader）已完整实现（缩放、JPEG DCT+量化编码），但作为 optional feature 默认关闭，用户必须手动 `--features gpu` 编译才能使用。这导致绝大多数用户无法享受 GPU 带来的 2-7x 性能提升。应将 GPU 加速设为默认启用，同时保持静默降级能力，让有 GPU 的用户开箱即用获得加速，无 GPU 的用户无感知地使用 CPU 回退。

## What Changes

- **BREAKING**: `Cargo.toml` 中 `default` features 从 `[]` 改为 `["gpu"]`，默认编译将包含 `wgpu`、`bytemuck`、`pollster` 依赖。不需要 GPU 的用户需使用 `--no-default-features` 编译
- 新增 UI 状态栏 GPU 指示器，显示当前加速状态（"GPU ✓ {适配器名}" 或 "CPU"）
- 利用 `AppState` global 中已有的 `gpu-available` 和 `gpu-name` 属性，由 Rust 侧在初始化后写入
- 降级策略保持不变：GPU 探测失败时静默回退到 CPU，仅通过 `tracing` 日志记录

## Capabilities

### New Capabilities

- `gpu-status-indicator`: UI 状态栏 GPU 状态指示器组件，显示当前是 GPU 加速还是 CPU 模式，以及 GPU 适配器名称

### Modified Capabilities

（无现有 specs 需要修改）

## Impact

- **Cargo.toml**: `default` features 变更，新增 `wgpu`/`bytemuck`/`pollster` 为默认依赖
- **编译体积**: release 二进制增大约 5-10MB（wgpu + 驱动层）
- **编译时间**: 首次编译增加（wgpu 编译较慢），增量编译影响小
- **UI 层**: `ui/globals/state.slint` 新增属性，主窗口新增状态栏组件
- **Rust 侧**: `src/main.rs` 或 `src/app.rs` 需在初始化后将 GPU 状态写入 `AppState`
- **CI**: 默认构建将包含 GPU feature，CI 环境无 GPU 时自动降级（不影响功能测试）
- **向后兼容**: 使用 `--no-default-features` 可恢复纯 CPU 构建
