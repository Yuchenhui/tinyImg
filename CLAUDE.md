# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 项目概述

TinyImg 是一个跨平台桌面图片压缩工具，使用纯 Rust 技术栈构建。支持 JPEG / PNG / WebP / AVIF / JPEG XL / GIF / SVG 七种格式的压缩与格式转换，提供批量处理、预设管理、实时预览和可选 GPU 加速能力。

## 技术栈

| 层级 | 技术 | 说明 |
|------|------|------|
| GUI | Slint (Skia backend) | 声明式 .slint DSL，纯 Rust 原生渲染 |
| 语言 | Rust edition 2021 | MSRV 1.85+ |
| JPEG | mozjpeg-rs | 纯 Rust 实现，仅编码器 |
| PNG 无损 | oxipng | 多线程无损优化 |
| PNG 有损 | imagequant v4 | 纯 Rust，**GPL v3** |
| WebP | image-webp | 纯 Rust，image-rs 生态 |
| AVIF | ravif (rav1e) | 纯 Rust 编码 |
| JPEG XL | jxl-oxide | 纯 Rust |
| GIF | gifski | 纯 Rust，**AGPL v3** |
| SVG | svgo | JS，通过 sidecar 或 WASM 集成 |
| 图像基础 | image-rs 0.25 + fast_image_resize 5 | 解码/基础操作 + SIMD 加速缩放 |
| GPU 加速 | wgpu (可选 feature) | compute shader 加速缩放/色彩转换/DCT |
| 并行 | rayon | 批量图片并行处理 |
| 序列化 | serde + toml | 配置/预设持久化 |
| 错误处理 | thiserror + anyhow | 库层用 thiserror，应用层用 anyhow |
| 日志 | tracing + tracing-subscriber | 结构化日志 |

## 项目目录结构

```
tinyimg/
├── Cargo.toml                  # 依赖配置，features 包含 gpu / svg-sidecar
├── build.rs                    # Slint 编译 + WGSL shader 嵌入
├── ui/                         # Slint UI 层
│   ├── app.slint               # 主窗口入口，import 所有页面
│   ├── widgets/                # 可复用组件
│   │   ├── drop_zone.slint     # 拖拽上传区
│   │   ├── image_card.slint    # 图片卡片（含缩略图/状态/压缩比）
│   │   ├── progress_bar.slint  # 进度条
│   │   ├── before_after.slint  # 压缩前后对比滑块
│   │   ├── format_badge.slint  # 格式标签
│   │   └── slider_input.slint  # 滑块+数值输入复合组件
│   ├── pages/                  # 页面
│   │   ├── compress.slint      # 主压缩页面
│   │   ├── settings.slint      # 设置/预设页面
│   │   └── about.slint         # 关于页面
│   ├── theme/                  # 主题定义
│   │   ├── palette.slint       # 颜色系统（亮/暗）
│   │   └── typography.slint    # 字体规格
│   └── globals/                # Slint global 单例（UI-Rust 桥接层）
│       ├── bridge.slint        # AppBridge: 所有 UI→Rust callback
│       └── state.slint         # AppState: 所有 Rust→UI property
├── src/
│   ├── main.rs                 # 入口：初始化 Slint 窗口，注册 callback，启动事件循环
│   ├── app.rs                  # App 结构体：持有 engine/config/gpu，连接 UI bridge
│   ├── engine/                 # 压缩引擎核心（可独立于 GUI 使用）
│   │   ├── mod.rs              # 导出 Pipeline, CodecRegistry, RawImage
│   │   ├── pipeline.rs         # CompressionPipeline: decode → preprocess chain → encode
│   │   ├── registry.rs         # CodecRegistry: 编解码器自动发现与注册
│   │   ├── raw_image.rs        # RawImage: 内部统一图像表示（封装 DynamicImage）
│   │   ├── params.rs           # EncodeParams, OutputFormat, QualityPreset 等参数类型
│   │   ├── codec/              # 编解码器实现（每个文件对应一种格式）
│   │   │   ├── mod.rs          # Codec / Decoder / Encoder trait 定义
│   │   │   ├── jpeg.rs         # MozjpegEncoder: mozjpeg-rs 封装
│   │   │   ├── png.rs          # OxipngEncoder + ImagequantQuantizer
│   │   │   ├── webp.rs         # WebpEncoder: image-webp 封装
│   │   │   ├── avif.rs         # AvifEncoder: ravif 封装
│   │   │   ├── jxl.rs          # JxlEncoder: jxl-oxide 封装
│   │   │   ├── gif.rs          # GifskiEncoder: gifski 封装
│   │   │   └── svg.rs          # SvgOptimizer: svgo sidecar/WASM 调用
│   │   └── preprocess/         # 预处理器（均实现 Preprocessor trait）
│   │       ├── mod.rs          # Preprocessor trait 定义
│   │       ├── resize.rs       # ResizeProcessor: fast_image_resize 封装
│   │       ├── color.rs        # ColorConverter: 色彩空间转换
│   │       ├── quantize.rs     # PaletteQuantizer: imagequant 量化（PNG 有损前置步骤）
│   │       └── metadata.rs     # MetadataStripper: EXIF/ICC 剥离
│   ├── gpu/                    # GPU 加速层（feature = "gpu"）
│   │   ├── mod.rs              # GpuAccelerator: wgpu device/queue 生命周期管理
│   │   ├── context.rs          # GpuContext: adapter 探测，自动降级
│   │   ├── resize.rs           # GpuResizeProcessor: impl Preprocessor，替换 CPU 版本
│   │   ├── color.rs            # GpuColorConverter: impl Preprocessor
│   │   └── dct.rs              # GpuDct: JPEG DCT 变换加速
│   ├── worker/                 # 工作线程管理
│   │   ├── mod.rs              # TaskManager: 任务队列 + rayon 调度
│   │   ├── task.rs             # CompressionTask, TaskStatus, TaskResult
│   │   └── progress.rs         # ProgressReporter: channel → UI 线程的进度桥接
│   ├── config/                 # 配置和预设管理
│   │   ├── mod.rs              # AppConfig: 全局配置（输出目录、语言、主题等）
│   │   ├── preset.rs           # CompressionPreset: 内置预设 + 用户自定义预设
│   │   └── storage.rs          # ConfigStorage: dirs crate 定位配置目录，TOML 读写
│   └── i18n/                   # 国际化
│       ├── mod.rs              # 语言切换逻辑，加载 .po/.ftl 文件
│       └── messages.rs         # 后端错误/状态消息的 enum → 翻译映射
├── locales/                    # 翻译文件（Slint @tr() 使用 gettext .po 格式）
│   ├── en/                     # English (default)
│   ├── zh-CN/                  # 简体中文
│   ├── zh-TW/                  # 繁体中文
│   └── ja/                     # 日本語
├── shaders/                    # WGSL compute shaders
│   ├── resize_bilinear.wgsl    # 双线性插值缩放
│   ├── resize_lanczos.wgsl     # Lanczos3 缩放
│   ├── color_convert.wgsl      # RGB↔YCbCr/Lab 色彩转换
│   └── dct_forward.wgsl        # 8x8 块 DCT 正变换
├── assets/                     # 应用资源
│   ├── icons/                  # 应用图标 (ico/png/svg)
│   └── fonts/                  # 内嵌字体（中日韩支持）
└── tests/                      # 集成测试
    ├── fixtures/               # 测试用图片样本
    └── pipeline_tests.rs       # 端到端压缩管线测试
```

## 构建与运行

```bash
# 开发运行（debug 模式，Slint live preview 可用）
cargo run

# 发布构建（启用 LTO + strip）
cargo build --release

# 启用 GPU 加速构建
cargo build --release --features gpu

# 运行测试
cargo test

# 运行特定模块测试
cargo test engine::codec::jpeg

# Clippy 检查
cargo clippy --all-features -- -D warnings

# 格式化
cargo fmt
```

**Slint Live Preview**: 安装 VS Code Slint 扩展后，编辑 .slint 文件时可实时预览 UI。

**Windows 构建前置**: 需要 Visual Studio Build Tools 2022（C++ 工作负载）。如果使用 `mozjpeg-sys` FFI 版本还需 NASM。当前选型 `mozjpeg-rs` 纯 Rust 无需 NASM。

## 架构设计

### 1. Slint UI 层与 Rust 后端通信

Slint 不使用 IPC/序列化，UI 和后端在同一进程内通过类型安全的 API 直接通信。

**桥接模式**：使用两个 Slint `global` 单例作为 UI↔Rust 边界：

```
┌─────────────────────────────────────────────────┐
│  .slint UI 层                                    │
│                                                  │
│  AppBridge (global)          AppState (global)   │
│  ├── callback add-files()    ├── in-out images:  │
│  ├── callback compress()     │   [ImageItem]     │
│  ├── callback cancel()       ├── in-out progress │
│  ├── callback remove(id)     ├── in-out status   │
│  └── callback save-preset()  └── in-out presets  │
│       ↑ UI 调用                    ↓ Rust 写入    │
├─────────────────────────────────────────────────┤
│  Rust 侧（main.rs / app.rs）                     │
│                                                  │
│  on_add_files(move || { ... })   ← 注册 callback │
│  state.set_progress(0.5)         ← 更新 property │
│  model.push(item)                ← 更新列表      │
└─────────────────────────────────────────────────┘
```

**线程安全约束**：Slint UI 对象只能在主线程访问。后台线程完成压缩后，必须通过以下方式回到主线程：

```rust
// 从工作线程更新 UI
let ui_handle = ui.as_weak();
slint::invoke_from_event_loop(move || {
    let ui = ui_handle.unwrap();
    ui.global::<AppState>().set_progress(0.75);
});
```

**图片列表数据驱动**：使用 `slint::VecModel<ImageItem>` 作为图片列表的数据源。UI 侧通过 `for item in AppState.images` 渲染，Rust 侧通过 `push / remove / set_row_data` 增删改。

### 2. 压缩管线（Pipeline）

参考 Rimage 架构，管线由三个阶段组成，每阶段解耦：

```
Input(PathBuf)
  │
  ▼
┌─────────────────────────────────────────────┐
│ Stage 1: Decode                              │
│ CodecRegistry::get_decoder(format) → decode  │
│ 输出: RawImage (封装 DynamicImage + metadata) │
└──────────────────┬──────────────────────────┘
                   ▼
┌─────────────────────────────────────────────┐
│ Stage 2: Preprocess Chain                    │
│ Vec<Box<dyn Preprocessor>> 按序执行          │
│ 可包含: Resize → ColorConvert → Quantize     │
│ GPU 实现和 CPU 实现共享 Preprocessor trait    │
└──────────────────┬──────────────────────────┘
                   ▼
┌─────────────────────────────────────────────┐
│ Stage 3: Encode                              │
│ CodecRegistry::get_encoder(output_format)    │
│ 输出: Vec<u8> (压缩后字节)                    │
└─────────────────────────────────────────────┘
```

**Pipeline 是无状态的**：每次压缩任务构造一个 Pipeline 实例（或克隆配置），传入 rayon 线程池执行。Pipeline 不持有 UI 引用。

**RawImage 设计**：

```rust
pub struct RawImage {
    pub pixels: DynamicImage,      // image-rs 像素数据
    pub source_format: ImageFormat, // 原始格式（用于 OutputFormat::Original）
    pub metadata: ImageMetadata,    // EXIF/ICC/XMP（可选保留）
    pub source_path: PathBuf,       // 原始文件路径
}
```

### 3. Codec trait 体系

```rust
/// 所有编解码器的基础信息
pub trait Codec: Send + Sync + 'static {
    fn name(&self) -> &'static str;
    fn formats(&self) -> &[ImageFormat];
}

/// 解码器：bytes → RawImage
pub trait Decoder: Codec {
    fn decode(&self, data: &[u8]) -> Result<RawImage>;
}

/// 编码器：RawImage → bytes，接受格式特定参数
pub trait Encoder: Codec {
    fn encode(&self, image: &RawImage, params: &EncodeParams) -> Result<EncodedOutput>;
}

/// 预处理器：RawImage → RawImage（原地变换）
pub trait Preprocessor: Send + Sync {
    fn name(&self) -> &'static str;
    fn process(&self, image: RawImage) -> Result<RawImage>;
}
```

**EncodeParams 使用 enum 而非 trait object**，因为每种格式参数差异大：

```rust
pub enum EncodeParams {
    Jpeg { quality: u8, progressive: bool },
    Png { lossy: bool, optimization_level: u8 },  // lossy=true 时先走 imagequant
    WebP { quality: u8, lossless: bool },
    Avif { quality: u8, speed: u8 },               // speed: 1(慢/高质量)~10(快/低质量)
    Jxl { quality: u8, effort: u8 },
    Gif { quality: u8, fast: bool },
    Svg { multipass: bool, precision: u8 },
    Passthrough,                                    // 保持原格式原参数
}
```

**CodecRegistry**：

```rust
pub struct CodecRegistry {
    decoders: HashMap<ImageFormat, Box<dyn Decoder>>,
    encoders: HashMap<ImageFormat, Box<dyn Encoder>>,
}

impl CodecRegistry {
    pub fn new() -> Self {
        let mut r = Self::default();
        // 注册所有内置编解码器
        r.register_decoder(ImageFormat::Jpeg, Box::new(JpegDecoder));
        r.register_encoder(ImageFormat::Jpeg, Box::new(MozjpegEncoder));
        r.register_encoder(ImageFormat::Png,  Box::new(OxipngEncoder));
        // ... 其他格式
        r
    }
}
```

### 4. GPU 加速层（wgpu）集成

**设计原则**：GPU 是 Preprocessor trait 的一种实现，与 CPU 实现可互换。

```
                    ┌─────────────────────┐
                    │  Preprocessor trait  │
                    └──────┬──────────────┘
                    ┌──────┴──────┐
              ┌─────┴─────┐ ┌────┴──────┐
              │ CpuResize │ │ GpuResize │
              │ (fast_    │ │ (wgpu     │
              │  image_   │ │  compute  │
              │  resize)  │ │  shader)  │
              └───────────┘ └───────────┘
```

**自动降级策略**：

```rust
pub fn create_resize_processor(gpu: Option<&GpuAccelerator>) -> Box<dyn Preprocessor> {
    match gpu {
        Some(gpu) if gpu.supports_compute() => Box::new(GpuResizeProcessor::new(gpu)),
        _ => Box::new(CpuResizeProcessor::new()),
    }
}
```

**GPU 上下文生命周期**：
- App 启动时异步探测 GPU（`GpuAccelerator::try_new().await`），不阻塞 UI
- 探测失败则标记为不可用，全程使用 CPU fallback
- GpuAccelerator 持有 `Arc<wgpu::Device>` 和 `Arc<wgpu::Queue>`，在多个 Pipeline 间共享
- compute shader 在 build.rs 中通过 `include_str!` 嵌入二进制

**适合 GPU 加速的操作**（Phase 2+）：
- 图像缩放（Resize）：8x8 工作组，双线性/Lanczos 插值
- 色彩空间转换：逐像素并行，RGB↔YCbCr/Lab
- DCT 正变换：JPEG 编码前的 8x8 块变换
- 实时预览缩略图生成

**不适合 GPU 的操作**：哈夫曼编码、Deflate 压缩、LZ 字典匹配（本质串行）。

### 5. 工作线程池和进度报告

```
┌───────────────────────────────────────────────────────┐
│  主线程 (UI 事件循环)                                  │
│                                                       │
│  ┌─────────┐    slint::invoke_from_event_loop()       │
│  │ Slint   │◄────────────────────────────────────┐    │
│  │ EventLoop│                                     │    │
│  └─────────┘                                     │    │
│       │ callback: compress()                     │    │
│       ▼                                          │    │
│  TaskManager::submit(tasks, params)              │    │
│       │ spawn background thread                  │    │
├───────┼──────────────────────────────────────────┤    │
│       ▼                                          │    │
│  ┌──────────────────────────────────────────┐    │    │
│  │ 后台线程 (rayon thread pool)              │    │    │
│  │                                           │    │    │
│  │  tasks.par_iter().for_each(|task| {       │    │    │
│  │      let result = pipeline.run(task);     │    │    │
│  │      progress_tx.send(update);  ──────────┼────┘    │
│  │  });                                      │         │
│  └──────────────────────────────────────────┘         │
└───────────────────────────────────────────────────────┘
```

**TaskManager** 负责：
1. 接收 UI 传来的文件列表和压缩参数
2. 在 `std::thread::spawn` 的后台线程中启动 rayon 并行处理
3. 每个 task 完成后通过 `slint::invoke_from_event_loop` 更新对应 ImageItem 的状态
4. 支持取消：通过 `AtomicBool` 标志位，rayon 迭代体内检查

**TaskStatus 状态机**：

```
Pending → Processing → Completed { original_size, compressed_size, output_path }
                    └→ Failed { error }
           ↑ (retry)
           └──────────┘
```

### 6. i18n 实现

**双层方案**：

| 层级 | 方案 | 格式 | 用途 |
|------|------|------|------|
| UI 文本 | Slint `@tr()` 宏 | gettext .po | 按钮、标签、提示等 |
| 后端消息 | rust-i18n `t!()` 宏 | YAML | 错误信息、状态文本 |

**语言切换流程**：用户在设置中选择语言 → 更新 AppConfig → 调用 `slint::set_locale()` + `rust_i18n::set_locale()` → UI 自动刷新。

**翻译文件位置**：`locales/{lang}/LC_MESSAGES/tinyimg.po`（UI）和 `locales/{lang}.yml`（后端）。

### 7. 配置和预设管理

**存储位置**：`dirs::config_dir() / "tinyimg" / "config.toml"`

**配置层次**：

```
AppConfig (全局)
├── language: String              # "zh-CN" / "en" / "ja"
├── theme: Theme                  # Light / Dark / System
├── output_dir: OutputDir         # SameAsInput / Custom(PathBuf)
├── overwrite: bool               # 是否覆盖原文件
├── suffix: String                # 输出文件后缀，如 "_compressed"
├── default_preset: String        # 默认预设名称
└── presets: Vec<CompressionPreset>

CompressionPreset
├── name: String                  # "Web 优化" / "高质量" / "最小体积"
├── jpeg: JpegParams              # quality, progressive
├── png: PngParams                # lossy, optimization_level
├── webp: WebpParams              # quality, lossless
├── avif: AvifParams              # quality, speed
├── gif: GifParams                # quality, fast
├── jxl: JxlParams                # quality, effort
├── resize: Option<ResizeParams>  # max_width, max_height, maintain_aspect
└── strip_metadata: bool
```

**内置预设**（不可删除，可覆盖参数）：
- **Web 优化**：JPEG q=80 progressive / PNG lossy o=2 / WebP q=80 → 平衡压缩率和质量
- **高质量**：JPEG q=92 / PNG lossless o=4 / WebP q=90 → 最小视觉损失
- **最小体积**：JPEG q=60 / PNG lossy o=6 / WebP q=50 → 激进压缩
- **无损**：所有格式使用无损模式（PNG lossless / WebP lossless / AVIF lossless）

## 关键设计决策

| 决策 | 选择 | 理由 |
|------|------|------|
| 单 crate vs workspace | 单 crate 起步 | 初期复杂度低；engine 模块已通过 pub 接口隔离，后续可无痛拆为 workspace |
| RawImage 用 DynamicImage | 封装而非重新实现 | image-rs 已处理格式探测/解码/像素布局，无需重复造轮子 |
| EncodeParams 用 enum | 而非 trait object / 泛型 | 格式参数差异大且固定，enum 匹配比动态派发更清晰，IDE 补全更好 |
| 进度报告用 invoke_from_event_loop | 而非 channel + Timer poll | Slint 官方推荐方式，避免轮询开销，即时更新 |
| GPU 层实现 Preprocessor trait | 而非独立接口 | 允许 CPU/GPU 实现透明互换，Pipeline 代码无需感知加速方式 |
| 预设存 TOML | 而非 JSON/YAML | Rust 生态一等公民，人类可读可编辑，serde_toml 零配置 |
| SVG 用 sidecar | 而非嵌入 JS 运行时 | 避免在纯 Rust 项目中引入 V8/QuickJS 依赖，sidecar 用 pkg 打包 svgo |

## 许可证注意事项

本项目采用 **GPL 兼容开源策略**，因此可以直接链接以下 GPL/AGPL 库：

- **imagequant v4** (GPL v3)：用于 PNG 有损量化。如果未来需要改为宽松许可证，可用 `color_quant` crate (MIT) 替代，但量化质量会下降。
- **gifski** (AGPL v3)：用于高质量 GIF 编码。AGPL 要求网络服务也必须开源；桌面应用场景下效果等同 GPL。如需移除，可降级为 `gif` crate (MIT) 但失去高级量化能力。
- **Slint**：桌面应用免费使用（Royalty-Free Desktop License）。嵌入式/商业分发需购买许可。

**整体许可证传染链**：imagequant (GPL v3) + gifski (AGPL v3) → 整个项目必须以 AGPL v3（或更严格）发布。如果要用 MIT/Apache-2.0 发布，必须移除这两个依赖或将它们改为进程隔离（sidecar）。

## Cargo.toml 依赖配置

```toml
[package]
name = "tinyimg"
version = "0.1.0"
edition = "2021"
rust-version = "1.85"
license = "AGPL-3.0-or-later"

[dependencies]
# GUI
slint = "1"

# 图像基础
image = { version = "0.25", default-features = false, features = [
    "png", "jpeg", "webp", "gif", "tiff", "bmp", "ico"
] }
fast_image_resize = "5"

# 压缩编码器
mozjpeg-rs = "0.2"
oxipng = { version = "9", default-features = false }
imagequant = "4"                   # GPL v3
image-webp = "0.2"
ravif = "0.11"
jxl-oxide = "0.11"
gifski = "1.13"                    # AGPL v3

# 并行
rayon = "1.10"

# 序列化/配置
serde = { version = "1", features = ["derive"] }
toml = "0.8"

# 错误处理
thiserror = "2"
anyhow = "1"

# 日志
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }

# 平台目录
dirs = "6"

# i18n (后端消息)
rust-i18n = "3"

# GPU 加速 (可选)
wgpu = { version = "24", optional = true }

[features]
default = []
gpu = ["dep:wgpu"]

[build-dependencies]
slint-build = "1"

[profile.release]
opt-level = 3
lto = "thin"
strip = true
codegen-units = 1
```

## 核心 Trait/Struct 设计草案

以下是 `src/engine/codec/mod.rs` 中定义的核心抽象：

```rust
use crate::engine::raw_image::RawImage;
use crate::engine::params::{EncodeParams, ImageFormat};
use anyhow::Result;

/// 编解码器基础信息
pub trait Codec: Send + Sync + 'static {
    fn name(&self) -> &'static str;
    fn formats(&self) -> &[ImageFormat];
}

/// 解码器：原始字节 → RawImage
pub trait Decoder: Codec {
    fn decode(&self, data: &[u8], source_format: ImageFormat) -> Result<RawImage>;
}

/// 编码器：RawImage → 压缩后字节
pub trait Encoder: Codec {
    fn encode(&self, image: &RawImage, params: &EncodeParams) -> Result<EncodedOutput>;
}

/// 编码输出
pub struct EncodedOutput {
    pub data: Vec<u8>,
    pub format: ImageFormat,
}
```

`src/engine/preprocess/mod.rs`：

```rust
use crate::engine::raw_image::RawImage;
use anyhow::Result;

/// 预处理步骤（Pipeline 中按序执行）
pub trait Preprocessor: Send + Sync {
    fn name(&self) -> &'static str;
    fn process(&self, image: RawImage) -> Result<RawImage>;
}
```

`src/engine/pipeline.rs`：

```rust
use crate::engine::codec::{Decoder, Encoder, EncodedOutput};
use crate::engine::preprocess::Preprocessor;
use crate::engine::params::EncodeParams;
use anyhow::Result;
use std::path::Path;

pub struct CompressionPipeline {
    decoder: Box<dyn Decoder>,
    preprocessors: Vec<Box<dyn Preprocessor>>,
    encoder: Box<dyn Encoder>,
    params: EncodeParams,
}

impl CompressionPipeline {
    pub fn run(&self, input_path: &Path) -> Result<CompressionResult> {
        let data = std::fs::read(input_path)?;
        let format = detect_format(&data)?;
        let original_size = data.len() as u64;

        // Stage 1: Decode
        let mut image = self.decoder.decode(&data, format)?;

        // Stage 2: Preprocess chain
        for processor in &self.preprocessors {
            image = processor.process(image)?;
        }

        // Stage 3: Encode
        let output = self.encoder.encode(&image, &self.params)?;

        Ok(CompressionResult {
            original_size,
            compressed_size: output.data.len() as u64,
            data: output.data,
            output_format: output.format,
        })
    }
}

pub struct CompressionResult {
    pub original_size: u64,
    pub compressed_size: u64,
    pub data: Vec<u8>,
    pub output_format: ImageFormat,
}
```

`src/worker/task.rs`：

```rust
use std::path::PathBuf;

pub struct CompressionTask {
    pub id: u64,
    pub input_path: PathBuf,
    pub output_path: PathBuf,
}

pub enum TaskStatus {
    Pending,
    Processing,
    Completed {
        original_size: u64,
        compressed_size: u64,
    },
    Failed {
        error: String,
    },
}

impl TaskStatus {
    pub fn compression_ratio(&self) -> Option<f64> {
        match self {
            Self::Completed { original_size, compressed_size } => {
                Some(1.0 - (*compressed_size as f64 / *original_size as f64))
            }
            _ => None,
        }
    }
}
```
