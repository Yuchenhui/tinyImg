# TinyImg 跨平台图片压缩工具 - 技术研究报告

> 研究日期: 2026-03-09
> 目标: 为设计一个跨平台（主要面向 Windows）的图片压缩工具提供全面的技术方案分析

---

## 目录

1. [开源图片压缩库分析](#1-开源图片压缩库分析)
2. [GUI 框架对比](#2-gui-框架对比)
3. [GPU 加速可行性分析](#3-gpu-加速可行性分析)
4. [Rust 生态图片处理库](#4-rust-生态图片处理库)
5. [推荐技术栈](#5-推荐技术栈)
6. [项目架构设计](#6-项目架构设计)
7. [依赖管理与构建系统](#7-依赖管理与构建系统)
8. [许可证风险分析](#8-许可证风险分析)

---

## 1. 开源图片压缩库分析

### 1.1 oxipng (PNG 无损优化)

| 项目 | 详情 |
|------|------|
| 语言 | Rust (纯 Rust) |
| 许可证 | MIT |
| 最低 Rust 版本 | 1.85.1 |
| 集成方式 | Cargo 依赖 (crate) |

**核心特性:**
- 多线程无损 PNG/APNG 压缩优化器
- 6 个优化级别 (`-o 0` 到 `-o 6`), 默认 `-o 2` 兼顾速度与压缩率
- 支持 Zopfli 高级压缩 (`-z` 标志)
- 元数据剥离 (safe/all 模式)
- Alpha 通道优化, 改善带透明度图片的压缩效果
- 作为库使用时, 通过 `Options` 结构体 + `optimize()` 函数调用

**性能基准:**
- 比 OptiPNG 快 2.22 倍 (标准测试)
- 在优化级别 4 下比 OptiPNG 快 5.01 倍
- 比 pngcrush 更快: oxipng 在 8.9 秒内节省 1867KB vs pngcrush 在 22.5 秒内节省 1754KB

**评估:** 纯 Rust 实现, MIT 许可证, 集成极其简单。是 PNG 无损压缩的最佳选择, 无需任何 C/C++ 编译依赖。

---

### 1.2 pngquant / libimagequant (PNG 有损量化)

| 项目 | 详情 |
|------|------|
| 语言 | Rust (v4 纯 Rust 重写) |
| 许可证 | **GPL v3** (开源) / 商业许可证 (闭源) |
| 集成方式 | `imagequant` crate |

**核心特性:**
- 将 24/32 位 RGBA 图片转换为 8 位调色板图像
- 通常可减少 70% 文件大小, 同时保持高视觉质量
- 使用改进的 Median Cut 量化算法 + Voronoi 迭代 (K-means) 校色
- 保留完整 Alpha 透明度
- v4 版本已完全用 Rust 重写, 但仍导出 C 接口

**注意事项:**
- 库本身不处理图像编解码, 需要自行集成编码器
- **GPL v3 许可证意味着如果项目闭源需要购买商业许可证**
- 联系 kornel@pngquant.org 获取商业许可

**评估:** PNG 有损压缩的业界标准。纯 Rust v4 版本集成简单, 但 GPL 许可证是重要的法律考量。

---

### 1.3 mozjpeg (JPEG 压缩)

| 项目 | 详情 |
|------|------|
| 原始语言 | C |
| Rust 绑定 | 多种选择 |
| 许可证 | BSD (Modified) |

**Rust 集成方案 (三选一):**

| 方案 | crate 名称 | 特点 |
|------|-----------|------|
| FFI 绑定 | `mozjpeg-sys` | 直接绑定 C 库, 需要 nasm + C 编译器, 自动静态链接 |
| 安全包装 | `mozjpeg` (ImageOptim) | 在 mozjpeg-sys 上提供安全 Rust API |
| 纯 Rust | `mozjpeg-rs` | 100% 安全 Rust 实现, 仅编码器, 输出与 C 版本字节级一致 |

**推荐方案:** 使用 `mozjpeg` crate (ImageOptim/mozjpeg-rust), 它在 FFI 绑定上提供了安全的高层 API。如果希望避免 C 编译依赖, 可选 `mozjpeg-rs` 纯 Rust 实现 (仅编码)。

**评估:** JPEG 压缩的黄金标准。BSD 许可证友好, 多种集成方案可选。

---

### 1.4 svgo (SVG 优化)

| 项目 | 详情 |
|------|------|
| 语言 | **JavaScript (Node.js)** |
| 许可证 | MIT |
| 集成难度 | 需要 Node.js 运行时或替代方案 |

**核心特性:**
- 插件化架构, 可扩展优化能力
- 移除编辑器元数据、注释、隐藏元素、默认值等冗余信息
- 支持多遍优化 (multipass)
- 提供命令行和 JavaScript API

**Rust 项目集成方案:**
1. **Sidecar 方式**: 将 svgo 打包为独立 Node.js 可执行文件 (pkg/nexe), 作为 Tauri sidecar 调用
2. **WASM 方式**: svgo 可编译为 WASM, 在 WebView 前端直接运行
3. **Rust 替代**: 寻找 Rust 原生的 SVG 优化库 (目前生态不够成熟)

**评估:** SVG 优化的事实标准, 但 JavaScript 实现给纯 Rust 项目带来集成挑战。建议在 WebView 前端直接运行或使用 sidecar。

---

### 1.5 pngcrush (PNG 压缩)

| 项目 | 详情 |
|------|------|
| 语言 | C |
| 状态 | 成熟但开发不活跃 |

**与 oxipng 对比:**
- oxipng 在速度和压缩率上均优于 pngcrush
- pngcrush 是单线程, oxipng 是多线程
- oxipng 是纯 Rust, 集成更简单

**评估:** 已被 oxipng 全面超越, **不推荐在新项目中使用**。

---

### 1.6 UUI (HackPlan UI 组件库)

| 项目 | 详情 |
|------|------|
| 语言 | TypeScript/React |
| 许可证 | MIT |
| 状态 | 仍在重度开发中, 向后兼容性无保证 |

**核心特性:**
- 通用工具优先 React UI 库
- TypeScript 全类型安全 (需要 TS 3.8+)
- HOC 工具实现组件样式定制
- WAI-ARIA 1.2 无障碍合规

**评估:** 图压使用的 UI 库, 但仍在开发中且不稳定。如果选择 Tauri + Web 前端方案, 可以考虑, 但建议选择更成熟的 UI 库 (如 shadcn/ui, Radix UI, Ant Design 等)。

---

## 2. GUI 框架对比

### 2.1 综合对比表

| 特性 | Tauri 2.0 | Slint | egui/eframe | Dioxus | Wails |
|------|-----------|-------|-------------|--------|-------|
| **语言** | Rust + Web技术 | Rust (.slint DSL) | 纯 Rust | 纯 Rust (JSX风格) | Go + Web技术 |
| **渲染方式** | 系统 WebView | OpenGL/Skia/软件 | OpenGL/wgpu | WebView (WRY) | 系统 WebView |
| **打包体积** | ~600KB-2.5MB | 极小 (<300KB运行时) | 中等 | <5MB | ~8MB |
| **启动速度** | ~2秒 | 极快 | 极快 | 快 | 快 |
| **内存占用** | ~80MB | 极低 | 低 | 类似 Tauri | 低 |
| **跨平台** | Win/Mac/Linux/Mobile | Win/Mac/Linux/嵌入式 | Win/Mac/Linux/Web | Win/Mac/Linux/Mobile/Web | Win/Mac/Linux |
| **UI 丰富度** | 完整 Web 生态 | 内置组件+自定义 | 有限 (即时模式) | Web 生态 | 完整 Web 生态 |
| **学习曲线** | 低 (Web开发者) | 中 (需学 .slint DSL) | 低-中 | 中 (需学 Rust) | 低 (Go+Web) |
| **原生感** | 取决于前端实现 | 自绘 (可定制) | 自绘 (非原生) | 取决于前端实现 | 取决于前端实现 |
| **桌面成熟度** | 高 | 中 (持续改进中) | 高 (工具类应用) | 中 | 中-高 |
| **生态/社区** | 大 | 中 | 大 | 中-大 | 中 |
| **许可证** | MIT/Apache-2.0 | 免费桌面/GPL/商业 | MIT/Apache-2.0 | MIT/Apache-2.0 | MIT |
| **拖拽文件支持** | 支持 (tauri://drop) | 支持 | 支持 | 支持 | 支持 |
| **构建时间** | ~343秒 (首次) | 快 | 快 | 中 | ~12秒 |

### 2.2 详细分析

#### Tauri 2.0 (推荐首选)

**优势:**
- 最成熟的非 Electron 桌面框架, 社区活跃
- 利用系统 WebView (Windows 上使用 WebView2/Edge), 无需打包浏览器引擎
- 打包体积极小 (600KB-2.5MB), 比 Electron 小 90%+
- 前端可使用任何 Web 框架 (React, Vue, Svelte, Solid 等)
- 后端使用 Rust, 天然适合集成 Rust 图片处理库
- 支持 Tauri Commands (前后端通信) 和 Sidecar (运行外部程序)
- 文件拖拽支持 (tauri://drop 事件)
- Tauri 2.0 新增移动端支持

**劣势:**
- 依赖系统 WebView, 不同平台渲染可能有细微差异
- 首次 Rust 编译时间较长
- 前后端通信涉及序列化/反序列化开销

**适用场景:** 非常适合图片压缩工具这类需要丰富 UI 交互 + 强大后端计算的应用。

#### Slint

**优势:**
- 极低运行时占用 (<300KB RAM)
- 声明式 .slint DSL, 编译期优化
- 桌面应用免费 (Royalty-Free License)
- 支持 Skia/OpenGL/软件渲染后端
- Live Preview + VS Code 扩展

**劣势:**
- 桌面功能仍在完善中 (官方声明 "in progress")
- UI 组件生态不如 Web 技术丰富
- 复杂的自定义 UI 需要更多工作
- 社区相对较小

**适用场景:** 适合对性能和体积有极致要求的场景, 但 UI 丰富度是短板。

#### egui/eframe

**优势:**
- 即时模式 API, 编码简单直观
- 跨平台 + Web (WASM) 支持
- 无障碍支持 (AccessKit)
- 非常适合工具类应用

**劣势:**
- UI 不是原生外观
- API 不稳定, 升级可能破坏兼容性
- 复杂布局实现困难
- IME 支持有限 (影响中文输入)

**适用场景:** 适合快速原型和内部工具, 不太适合面向终端用户的产品。

#### Dioxus

**优势:**
- React 风格的 Rust API, 声明式 UI
- 底层使用 Tauri 的 WRY (WebView), 同样小体积
- 热重载支持
- 内置 TailwindCSS + Radix UI 组件
- 纯 Rust 全栈

**劣势:**
- 版本仍在 0.7, API 不够稳定
- 仍有 Virtual DOM 开销
- 社区和文档不如 Tauri 丰富

**适用场景:** 适合希望全栈使用 Rust 但又想要 Web 式开发体验的开发者。

#### Wails

**优势:**
- Go 语言, 构建速度极快 (~12秒 vs Tauri ~343秒)
- 简单易学, 一天可上手
- 自动生成 TypeScript 定义
- IPC 通信设计优秀

**劣势:**
- 后端是 Go 而非 Rust, 集成 Rust 图片库需要额外的 FFI 层
- 不支持交叉编译 (不能在 Windows 上构建 Linux 版)
- 生态比 Tauri 小
- 多窗口等功能仍在开发中

**适用场景:** 适合 Go 开发者, 但对于这个项目, 由于核心压缩库都是 Rust 生态, 使用 Go 会增加集成复杂度。

---

## 3. GPU 加速可行性分析

### 3.1 图片压缩各环节的 GPU 加速潜力

| 处理环节 | GPU 加速可行性 | 说明 |
|----------|---------------|------|
| **图像缩放/Resize** | **高** | 高度并行, GPU 天然优势, 可获 10-50x 加速 |
| **色彩空间转换** | **高** | RGB/YUV/CMYK 转换是逐像素操作, 非常适合 GPU |
| **图像滤波/预处理** | **高** | 卷积、锐化、去噪等滤波操作可充分利用 GPU 并行 |
| **JPEG DCT 变换** | **高** | 块级 DCT 变换高度并行, NVIDIA nvJPEG 已证实 |
| **JPEG 量化** | **高** | 逐块操作, 适合 GPU |
| **JPEG 哈夫曼编码** | **低** | 有序/熵编码本质串行, 不适合 GPU |
| **PNG 过滤** | **中** | 行级过滤有一定并行度 |
| **PNG Deflate/LZ 压缩** | **低** | 基于字典的压缩算法本质串行, 无硬件加速 |
| **PNG 量化 (调色板)** | **中** | K-means 迭代可在 GPU 上加速 |
| **WebP/AVIF 编码** | **中-低** | 基于视频编码, 部分 GPU 有硬件编码器 |
| **SVG 优化** | **不适用** | XML 文本操作, 不适合 GPU |

### 3.2 GPU 加速技术方案

#### wgpu (推荐)

**优势:**
- 跨平台: Vulkan (Linux/Windows/Android), Metal (macOS/iOS), DX12 (Windows), OpenGL ES
- 安全的 Rust API, 无 unsafe 代码
- 基于 WebGPU 标准, 未来可扩展到 WASM/浏览器
- 使用 WGSL 着色器语言 (或 SPIR-V)
- 活跃的开发和社区

**劣势:**
- WGSL 文档有限
- 与 CUDA 相比, 生态成熟度不足
- GPU 通信开销 (PCI-E 带宽瓶颈), 小图片可能反而更慢

**性能建议:**
- 1D 计算任务: 工作组大小 64
- 2D 图像任务: 8x8 工作组 (64 线程)
- 尽量保持数据在 GPU 内存中, 减少 CPU-GPU 传输

#### CUDA/nvJPEG (仅限 NVIDIA)

**优势:**
- NVIDIA nvJPEG 可实现 JPEG 解码加速 51 倍 (vs libjpeg-turbo)
- 硬件 JPEG 解码器 (A100 等)
- 生态最成熟

**劣势:**
- 仅支持 NVIDIA GPU
- 需要 CUDA Toolkit 运行时
- 增加分发复杂度

#### Vulkan Compute

**优势:**
- 比 wgpu 更底层, 性能天花板更高
- 广泛的 GPU 支持

**劣势:**
- API 极其复杂
- 开发工作量巨大
- Rust 绑定 (ash/vulkano) 使用门槛高

#### DirectX Compute (仅 Windows)

**优势:**
- Windows 原生支持
- 与 DirectX 12 集成

**劣势:**
- 仅限 Windows
- 与跨平台目标矛盾

### 3.3 GPU 加速实施建议

**结论: GPU 加速对于桌面图片压缩工具的投入产出比有限, 建议作为渐进式优化。**

**理由:**
1. 桌面图片压缩的典型场景是处理几十到几百张图片, 而非实时视频流
2. 现有 CPU 多线程库 (oxipng, mozjpeg, rayon) 已经足够快
3. PNG 压缩的核心 (Deflate) 无法有效 GPU 加速
4. GPU 数据传输开销可能抵消小图片的加速收益
5. 增加 GPU 依赖会显著复杂化分发

**推荐渐进式方案:**

```
Phase 1 (MVP): 纯 CPU 多线程, 使用 rayon 并行处理多张图片
Phase 2 (优化): 使用 wgpu 加速图像预处理 (缩放、色彩转换、滤波)
Phase 3 (高级): 可选 GPU 加速 JPEG DCT 变换和量化 (仅在检测到可用 GPU 时启用)
```

**适合 GPU 加速的具体场景:**
- 批量图片缩放 (Resize) -- 最有价值
- 批量色彩空间转换
- 图像滤波预处理 (锐化、去噪后压缩效果更好)
- 实时预览 (用 GPU 快速生成缩略图显示压缩效果对比)

---

## 4. Rust 生态图片处理库

### 4.1 各格式最佳压缩库推荐

| 格式 | 推荐库 | 类型 | 许可证 | 说明 |
|------|--------|------|--------|------|
| **PNG (无损)** | `oxipng` | 纯 Rust | MIT | 多线程, 最佳性能 |
| **PNG (有损)** | `imagequant` | 纯 Rust (v4) | GPL v3 / 商业 | 调色板量化, 需注意许可证 |
| **JPEG** | `mozjpeg` / `mozjpeg-rs` | FFI / 纯 Rust | BSD | 压缩率最优的 JPEG 编码器 |
| **WebP** | `image-webp` | 纯 Rust | MIT/Apache-2.0 | image-rs 生态, 纯 Rust |
| **AVIF** | `ravif` | 纯 Rust (rav1e) | BSD-2-Clause | 纯 Rust AVIF 编码 |
| **SVG** | `svgo` (JS) | Node.js | MIT | 需要 sidecar 或 WASM 集成 |
| **GIF** | `gifski` | 纯 Rust | AGPL v3 / 商业 | 最高质量 GIF, 基于 imagequant |
| **JPEG XL** | `jxl-oxide` / `jpegxl-rs` | 纯 Rust / FFI | BSD | 新兴格式, Chrome 145 已支持 |

### 4.2 核心库详解

#### image-rs (基础图像处理)

```toml
[dependencies]
image = "0.25"
```

- 统一的图像编解码接口
- 支持格式: PNG, JPEG, WebP, GIF, TIFF, BMP, ICO, AVIF 等
- 提供基础图像操作: 裁剪、缩放、旋转、色彩调整
- 稳定的 API 设计
- 可通过 feature flags 选择性启用格式支持

#### Rimage (已集成的优化工具)

```toml
[dependencies]
rimage = "0.11"
```

- 已集成 MozJPEG, OxiPNG, ravif 等编码器
- 支持输入: AVIF, BMP, JPEG, JPEG-XL, PNG, WebP, TIFF, PSD, QOI 等
- 支持输出: AVIF, JPEG, JPEG-XL, PNG, WebP, QOI 等
- 内置预处理管线: 缩放、量化、Alpha 预乘
- 使用 `fast_image_resize` 进行高性能缩放
- 许可证: MIT/Apache-2.0

**重要参考:** Rimage 的架构设计可以作为本项目核心压缩引擎的直接参考, 甚至可以直接作为依赖使用。

#### fast_image_resize

```toml
[dependencies]
fast_image_resize = "5"
```

- 使用 SIMD (SSE4.1, AVX2, NEON) 加速图像缩放
- 支持多种插值算法: Nearest, Bilinear, Bicubic, Lanczos3
- 比 image-rs 内置缩放快 3-10 倍

### 4.3 C/C++ 库集成到 Rust 的方法

#### 方法一: 使用现有的 `-sys` crate

```toml
# 直接使用社区维护的 FFI 绑定
[dependencies]
mozjpeg-sys = "4"      # mozjpeg FFI
```

- 优势: 开箱即用, 社区维护
- 劣势: 需要 C 编译器 (Windows 上需 MSVC 或 MinGW), 可能需要 nasm

#### 方法二: 使用安全封装 crate

```toml
# 使用高层安全 API
[dependencies]
mozjpeg = "0.10"  # ImageOptim 维护的安全封装
```

- 优势: 安全的 Rust API, 无需 unsafe
- 劣势: 仍然依赖底层 C 编译

#### 方法三: 纯 Rust 替代

```toml
# 完全避免 C 依赖
[dependencies]
mozjpeg-rs = "0.2"  # 纯 Rust mozjpeg (仅编码)
oxipng = "9"        # 纯 Rust (已经是)
imagequant = "4"    # 纯 Rust (v4 已经是)
```

- 优势: 零 C 依赖, 编译简单, 交叉编译友好
- 劣势: 部分功能可能不完整 (如 mozjpeg-rs 仅支持编码)

#### 方法四: CXX (C++ 互操作)

```toml
[dependencies]
cxx = "1.0"
[build-dependencies]
cxx-build = "1.0"
cc = "1.0"
```

- 优势: 类型安全的 C++ 互操作
- 劣势: 增加构建复杂度

**推荐策略:** 优先使用纯 Rust 实现, 仅在必要时 (性能或功能差距) 使用 FFI 绑定。

---

## 5. 推荐技术栈

### 5.1 方案 A: Tauri 2.0 + React (推荐方案)

```
前端: React + TypeScript + TailwindCSS + shadcn/ui
后端: Rust (Tauri Core)
压缩引擎: oxipng + mozjpeg + imagequant + ravif + image-webp + gifski
SVG: svgo (WASM 运行于前端, 或 sidecar)
图片处理: image-rs + fast_image_resize
并行: rayon
构建: Cargo + pnpm/bun (前端)
```

**推荐理由:**
1. **开发效率最高**: Web 前端开发者丰富, UI 库生态成熟
2. **后端集成最优**: Tauri 后端就是 Rust, 直接调用压缩库, 零额外 FFI 开销
3. **体积极小**: 比 Electron 小 90%+, 满足轻量需求
4. **社区最大**: Tauri 是目前最活跃的非 Electron 桌面框架
5. **文件操作友好**: 内置拖拽、文件对话框、文件系统 API
6. **扩展性强**: 未来可通过 Tauri 2.0 扩展到移动端

**数据流:**
```
用户拖拽/选择图片 → 前端 (React) → Tauri Command (IPC) → Rust 后端
→ 解码图片 (image-rs) → 预处理 (缩放/色彩转换) → 编码压缩
→ 返回结果 → 前端显示对比/保存
```

### 5.2 方案 B: Tauri 2.0 + Solid.js (轻量替代)

```
前端: Solid.js + TypeScript + TailwindCSS
后端: Rust (Tauri Core)
其他同方案 A
```

**优势:** Solid.js 比 React 更轻量, 性能更好 (无 Virtual DOM), 打包体积更小。
**劣势:** Solid.js 生态不如 React 成熟。

### 5.3 方案 C: Slint + Rust (纯 Rust 方案)

```
GUI: Slint (Skia 渲染后端)
压缩引擎: 同方案 A
并行: rayon
构建: Cargo
```

**优势:** 完全纯 Rust, 极致性能和体积, 单一语言栈。
**劣势:** UI 定制灵活度不如 Web 技术, 需要学习 .slint DSL, SVG 优化集成更复杂。

### 5.4 方案对比

| 维度 | 方案A (Tauri+React) | 方案B (Tauri+Solid) | 方案C (Slint) |
|------|-------------------|-------------------|-------------|
| 开发效率 | 最高 | 高 | 中 |
| 运行性能 | 高 | 更高 | 最高 |
| 打包体积 | ~2-3MB | ~2MB | ~1-2MB |
| UI 丰富度 | 最高 | 高 | 中 |
| 维护性 | 好 | 好 | 需要更多 Rust 经验 |
| 团队友好 | 最好 | 好 | 仅 Rust 开发者 |

**最终推荐: 方案 A (Tauri 2.0 + React)**

---

## 6. 项目架构设计

### 6.1 整体架构

```
tinyimg/
├── src-tauri/                    # Rust 后端 (Tauri)
│   ├── Cargo.toml
│   ├── tauri.conf.json
│   ├── src/
│   │   ├── main.rs              # Tauri 入口
│   │   ├── lib.rs               # 库导出
│   │   ├── commands/            # Tauri Commands (前后端桥接)
│   │   │   ├── mod.rs
│   │   │   ├── compress.rs      # 压缩相关命令
│   │   │   ├── preview.rs       # 预览相关命令
│   │   │   └── settings.rs      # 设置相关命令
│   │   ├── engine/              # 压缩引擎核心
│   │   │   ├── mod.rs
│   │   │   ├── pipeline.rs      # 压缩管线
│   │   │   ├── codecs/          # 各格式编解码器
│   │   │   │   ├── mod.rs
│   │   │   │   ├── jpeg.rs      # mozjpeg 封装
│   │   │   │   ├── png.rs       # oxipng + imagequant 封装
│   │   │   │   ├── webp.rs      # WebP 编码
│   │   │   │   ├── avif.rs      # AVIF 编码
│   │   │   │   ├── gif.rs       # GIF 优化
│   │   │   │   ├── jxl.rs       # JPEG XL 编码
│   │   │   │   └── svg.rs       # SVG 优化 (调用 sidecar/WASM)
│   │   │   ├── preprocess/      # 预处理
│   │   │   │   ├── mod.rs
│   │   │   │   ├── resize.rs    # 图像缩放
│   │   │   │   ├── color.rs     # 色彩空间转换
│   │   │   │   └── filter.rs    # 图像滤波
│   │   │   └── gpu/             # GPU 加速 (Phase 2+)
│   │   │       ├── mod.rs
│   │   │       ├── context.rs   # wgpu 上下文管理
│   │   │       ├── resize.rs    # GPU 缩放
│   │   │       └── shaders/     # WGSL 着色器
│   │   ├── worker/              # 工作线程管理
│   │   │   ├── mod.rs
│   │   │   ├── pool.rs          # 线程池
│   │   │   └── progress.rs      # 进度报告
│   │   └── config/              # 配置管理
│   │       ├── mod.rs
│   │       └── profiles.rs      # 压缩预设配置
│   └── icons/                   # 应用图标
├── src/                         # 前端 (React + TypeScript)
│   ├── App.tsx
│   ├── main.tsx
│   ├── components/
│   │   ├── DropZone.tsx         # 拖拽上传区域
│   │   ├── ImageList.tsx        # 图片列表
│   │   ├── ImageItem.tsx        # 单个图片项 (含对比预览)
│   │   ├── CompressSettings.tsx # 压缩设置面板
│   │   ├── ProgressBar.tsx      # 进度条
│   │   ├── BeforeAfter.tsx      # 压缩前后对比
│   │   └── FormatSelector.tsx   # 输出格式选择
│   ├── hooks/
│   │   ├── useCompress.ts       # 压缩操作 hook
│   │   ├── useDragDrop.ts       # 拖拽 hook
│   │   └── useSettings.ts      # 设置 hook
│   ├── stores/                  # 状态管理
│   │   ├── imageStore.ts
│   │   └── settingsStore.ts
│   ├── types/                   # TypeScript 类型定义
│   │   └── index.ts
│   └── utils/
│       └── tauri.ts             # Tauri API 封装
├── package.json
├── tsconfig.json
├── vite.config.ts
└── tailwind.config.js
```

### 6.2 核心数据流设计

```
┌──────────────────────────────────────────────────────────┐
│                    Frontend (WebView)                      │
│  ┌─────────┐    ┌──────────┐    ┌──────────────────────┐ │
│  │ DropZone │───→│ ImageList │───→│ CompressSettings     │ │
│  └─────────┘    └──────────┘    └──────────────────────┘ │
│       │              │                     │               │
│       └──────────────┴─────────────────────┘               │
│                        │ Tauri IPC                         │
├────────────────────────┼──────────────────────────────────┤
│                        ↓         Backend (Rust)            │
│  ┌─────────────────────────────────────────────────────┐  │
│  │                  Tauri Commands                      │  │
│  │  compress_images() / get_preview() / save_results()  │  │
│  └──────────────────────┬──────────────────────────────┘  │
│                         ↓                                  │
│  ┌─────────────────────────────────────────────────────┐  │
│  │              Compression Pipeline                    │  │
│  │                                                      │  │
│  │  Input → Decode → Preprocess → Encode → Output      │  │
│  │           (image-rs)  (resize,   (mozjpeg,           │  │
│  │                       color)     oxipng,             │  │
│  │                                  ravif...)           │  │
│  └──────────────────────┬──────────────────────────────┘  │
│                         ↓                                  │
│  ┌─────────────────────────────────────────────────────┐  │
│  │              Worker Thread Pool (rayon)               │  │
│  │  ┌────────┐ ┌────────┐ ┌────────┐ ┌────────┐       │  │
│  │  │Worker 1│ │Worker 2│ │Worker 3│ │Worker N│       │  │
│  │  └────────┘ └────────┘ └────────┘ └────────┘       │  │
│  └─────────────────────────────────────────────────────┘  │
│                         ↓ (Phase 2+)                       │
│  ┌─────────────────────────────────────────────────────┐  │
│  │              GPU Accelerator (wgpu) [可选]            │  │
│  │  Resize / Color Conversion / Preview Generation      │  │
│  └─────────────────────────────────────────────────────┘  │
└──────────────────────────────────────────────────────────┘
```

### 6.3 压缩管线设计 (Pipeline)

```rust
// 伪代码示意
pub struct CompressionPipeline {
    preprocess: Vec<Box<dyn Preprocessor>>,
    encoder: Box<dyn Encoder>,
    options: CompressionOptions,
}

pub struct CompressionOptions {
    pub format: OutputFormat,         // 输出格式
    pub quality: u8,                  // 质量 (0-100)
    pub max_width: Option<u32>,       // 最大宽度
    pub max_height: Option<u32>,      // 最大高度
    pub strip_metadata: bool,         // 剥离元数据
    pub preserve_aspect_ratio: bool,  // 保持宽高比
    pub target_size: Option<u64>,     // 目标文件大小
}

pub enum OutputFormat {
    Jpeg { progressive: bool },
    Png { lossy: bool, optimization_level: u8 },
    WebP { lossless: bool },
    Avif { speed: u8 },
    Gif { lossy: bool },
    JpegXl,
    Svg,
    Original, // 保持原格式
}
```

### 6.4 关键特性实现

#### 拖拽文件处理

```typescript
// 前端: 监听 Tauri 拖拽事件
import { listen } from '@tauri-apps/api/event';

listen('tauri://drop', (event) => {
  const files: string[] = event.payload as string[];
  // 将文件路径发送到后端进行处理
  invoke('add_images', { paths: files });
});
```

#### 压缩进度报告

```rust
// 后端: 通过 Tauri 事件向前端报告进度
use tauri::Emitter;

fn compress_with_progress(app: &AppHandle, images: Vec<ImageTask>) {
    let total = images.len();
    images.par_iter().enumerate().for_each(|(i, image)| {
        let result = compress_single(image);
        app.emit("compress-progress", ProgressPayload {
            current: i + 1,
            total,
            filename: image.filename.clone(),
            result,
        }).unwrap();
    });
}
```

#### 压缩前后对比预览

```rust
// 后端: 生成压缩预览
#[tauri::command]
async fn get_preview(
    path: String,
    options: CompressionOptions,
) -> Result<PreviewResult, String> {
    let original = image::open(&path).map_err(|e| e.to_string())?;
    let compressed = compress_image(&original, &options)?;

    Ok(PreviewResult {
        original_size: fs::metadata(&path).map(|m| m.len()).unwrap_or(0),
        compressed_size: compressed.len() as u64,
        // 返回 base64 缩略图用于前端显示
        preview_thumbnail: base64_encode(&generate_thumbnail(&compressed)),
    })
}
```

---

## 7. 依赖管理与构建系统

### 7.1 Cargo.toml 核心依赖

```toml
[package]
name = "tinyimg"
version = "0.1.0"
edition = "2021"
rust-version = "1.85"

[dependencies]
# Tauri
tauri = { version = "2", features = ["devtools"] }
tauri-plugin-dialog = "2"
tauri-plugin-fs = "2"
tauri-plugin-shell = "2"

# 图像处理核心
image = { version = "0.25", default-features = false, features = [
    "png", "jpeg", "webp", "gif", "tiff", "bmp"
] }

# 压缩编码器
oxipng = { version = "9", default-features = false }
mozjpeg = "0.10"              # 或 mozjpeg-rs (纯 Rust)
ravif = "0.11"                # AVIF 编码
image-webp = "0.2"            # WebP 编码/解码

# PNG 量化 (注意 GPL 许可证)
imagequant = "4"

# GIF 优化
gifski = "1.13"               # 注意 AGPL 许可证

# 图像缩放
fast_image_resize = "5"

# 并行处理
rayon = "1.10"

# 序列化
serde = { version = "1", features = ["derive"] }
serde_json = "1"

# 错误处理
thiserror = "2"
anyhow = "1"

# 日志
tracing = "0.1"
tracing-subscriber = "0.3"

# GPU 加速 (可选, Phase 2)
# wgpu = { version = "24", optional = true }

[features]
default = []
gpu = ["wgpu"]
```

### 7.2 前端 package.json

```json
{
  "name": "tinyimg",
  "private": true,
  "type": "module",
  "scripts": {
    "dev": "vite",
    "build": "tsc && vite build",
    "tauri": "tauri"
  },
  "dependencies": {
    "@tauri-apps/api": "^2",
    "react": "^19",
    "react-dom": "^19",
    "zustand": "^5",
    "@radix-ui/react-slider": "^1",
    "@radix-ui/react-select": "^2",
    "@radix-ui/react-dialog": "^1"
  },
  "devDependencies": {
    "@tauri-apps/cli": "^2",
    "typescript": "^5.7",
    "vite": "^6",
    "@vitejs/plugin-react": "^4",
    "tailwindcss": "^4",
    "autoprefixer": "^10"
  }
}
```

### 7.3 构建系统建议

#### Windows 构建要求

```
1. Rust 工具链: rustup (stable channel, MSVC target)
2. Visual Studio Build Tools 2022 (C++ 工作负载)
3. NASM (用于 mozjpeg-sys 编译, 如使用 FFI 版本)
4. Node.js 20+ 和 pnpm (前端构建)
5. WebView2 Runtime (Windows 10/11 已内置)
```

#### CI/CD (GitHub Actions)

```yaml
# .github/workflows/build.yml
name: Build
on: [push, pull_request]
jobs:
  build-windows:
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: pnpm/action-setup@v4
      - uses: actions/setup-node@v4
        with:
          node-version: 20
      - run: pnpm install
      - run: pnpm tauri build
```

#### 交叉编译注意

- 使用纯 Rust 库 (mozjpeg-rs, oxipng, imagequant v4) 可简化交叉编译
- 如使用 mozjpeg-sys (C FFI), 需要为每个目标平台配置交叉编译工具链
- Tauri 2.0 支持 `tauri build --target` 指定目标平台

---

## 8. 许可证风险分析

### 8.1 许可证汇总

| 库 | 许可证 | 闭源项目可用性 | 备注 |
|-----|--------|--------------|------|
| oxipng | MIT | 可以 | 无限制 |
| mozjpeg / mozjpeg-sys | BSD | 可以 | 无限制 |
| mozjpeg-rs | MIT/Apache-2.0 | 可以 | 无限制 |
| image-rs | MIT/Apache-2.0 | 可以 | 无限制 |
| ravif | BSD-2-Clause | 可以 | 无限制 |
| image-webp | MIT/Apache-2.0 | 可以 | 无限制 |
| fast_image_resize | MIT/Apache-2.0 | 可以 | 无限制 |
| Tauri | MIT/Apache-2.0 | 可以 | 无限制 |
| svgo | MIT | 可以 | 无限制 |
| Slint | 免费 (桌面) | 可以 | 桌面免费, 嵌入式收费 |
| **imagequant** | **GPL v3** | **需商业许可** | 联系 kornel@pngquant.org |
| **gifski** | **AGPL v3** | **需商业许可** | 更严格的 GPL 变种 |

### 8.2 许可证策略建议

**如果项目开源 (GPL 兼容):**
- 直接使用 imagequant 和 gifski, 无许可证问题

**如果项目闭源或宽松许可:**
- **方案一:** 购买 imagequant 和 gifski 的商业许可证
- **方案二:** 将 imagequant/gifski 作为可选外部工具 (sidecar), 不与主程序链接
- **方案三:** 寻找替代库:
  - PNG 量化: 可以使用 `color_quant` crate (MIT) 替代, 但质量可能不如 imagequant
  - GIF 优化: 可以使用 `gif` crate (MIT/Apache-2.0) 做基础 GIF 编码, 放弃高级量化

---

## 总结与行动建议

### 核心结论

1. **GUI 框架**: Tauri 2.0 是最优选择 -- 兼具轻量、性能、生态三大优势, 完美替代 Electron
2. **压缩引擎**: 以 Rust 生态为核心, oxipng + mozjpeg + ravif + image-webp 构成无许可证风险的基础方案
3. **GPU 加速**: 投入产出比有限, 建议 Phase 2 引入 wgpu 仅加速图像缩放和预处理
4. **Rimage 参考**: 该项目的架构设计 (管线模式、编解码器抽象) 值得深度参考
5. **许可证**: imagequant (GPL) 和 gifski (AGPL) 需要特别注意, 闭源项目需购买商业许可或使用替代方案

### 建议开发路线

```
Phase 1 - MVP (4-6 周):
├── Tauri 2.0 项目搭建
├── 基础 UI (拖拽、列表、设置面板)
├── JPEG 压缩 (mozjpeg)
├── PNG 无损优化 (oxipng)
├── 批量处理 + 进度显示
└── 压缩前后对比预览

Phase 2 - 格式扩展 (3-4 周):
├── WebP 支持 (image-webp)
├── AVIF 支持 (ravif)
├── PNG 有损压缩 (imagequant, 确认许可证)
├── 输出格式转换
└── 压缩预设配置

Phase 3 - 高级功能 (3-4 周):
├── SVG 优化 (svgo WASM/sidecar)
├── GIF 优化 (gifski, 确认许可证)
├── JPEG XL 支持 (jxl-oxide)
├── 目标文件大小压缩
└── GPU 加速图像缩放 (wgpu, 可选)

Phase 4 - 打磨 (2-3 周):
├── 性能优化
├── 多语言支持
├── 自动更新 (Tauri updater)
├── 安装包签名
└── 发布
```
