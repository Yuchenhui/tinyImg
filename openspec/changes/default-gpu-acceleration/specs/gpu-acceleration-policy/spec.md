## ADDED Requirements

### Requirement: Platform-specific GPU backend selection

当 `gpu` feature 启用时，wgpu 实例 SHALL 仅初始化目标平台对应的单一图形后端：Windows 使用 DX12，macOS 使用 Metal，Linux 使用 Vulkan。编译产物 MUST 包含 Vulkan 后端支持以确保 Linux 平台 GPU 可用。

#### Scenario: Windows uses DX12 backend

- **WHEN** 应用在 Windows 上启动且 `gpu` feature 已编译
- **THEN** `wgpu::Instance` MUST 仅使用 `Backends::DX12` 初始化

#### Scenario: macOS uses Metal backend

- **WHEN** 应用在 macOS 上启动且 `gpu` feature 已编译
- **THEN** `wgpu::Instance` MUST 仅使用 `Backends::METAL` 初始化

#### Scenario: Linux uses Vulkan backend

- **WHEN** 应用在 Linux 上启动且 `gpu` feature 已编译
- **THEN** `wgpu::Instance` MUST 仅使用 `Backends::VULKAN` 初始化
- **THEN** 编译产物 MUST 链接 Vulkan 相关库

#### Scenario: Vulkan feature included in default build

- **WHEN** 通过 `cargo build`（使用默认 features）编译
- **THEN** wgpu 依赖 MUST 包含 `vulkan` feature，确保 Linux 编译通过

### Requirement: JPEG encoding defaults to mozjpeg

JPEG 编码 SHALL 默认使用 mozjpeg CPU 编码器，即使 GPU 可用。`create_jpeg_encoder` 工厂函数在所有情况下 MUST 返回 `MozjpegEncoder` 实例。GPU JPEG 编码器（自建 DCT+量化管线）代码保留但不默认使用。

#### Scenario: GPU available but JPEG uses mozjpeg

- **WHEN** GPU 初始化成功（`gpu-available` 为 `true`）且用户压缩 JPEG 图片
- **THEN** 编码器 MUST 使用 mozjpeg（日志应显示 "Using CPU JPEG encoder (mozjpeg)"）
- **THEN** 输出文件体积 MUST 与纯 CPU 构建（`--no-default-features`）在相同 quality 值下一致

#### Scenario: GPU unavailable also uses mozjpeg

- **WHEN** GPU 不可用且用户压缩 JPEG 图片
- **THEN** 编码器 MUST 使用 mozjpeg，行为与 GPU 可用时完全一致

### Requirement: GPU acceleration limited to preprocessing

GPU 加速 SHALL 仅应用于预处理阶段（resize 缩放、色彩转换），不应用于最终编码阶段。GPU 预处理的输出与 CPU 预处理 SHALL 在像素级数值等价（允许浮点精度误差）。

#### Scenario: GPU resize produces equivalent output

- **WHEN** 同一张图片分别使用 GPU resize 和 CPU resize（fast_image_resize）缩放到相同目标尺寸
- **THEN** 两者输出的 PSNR MUST 不低于 40dB

#### Scenario: Preprocessing does not affect encoding choice

- **WHEN** GPU 用于图片预处理（缩放）后进入编码阶段
- **THEN** 编码器选择 MUST 不受预处理是否使用 GPU 影响（JPEG 始终用 mozjpeg）
