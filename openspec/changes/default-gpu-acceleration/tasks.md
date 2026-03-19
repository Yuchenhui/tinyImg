## 1. Cargo.toml 与构建配置

- [ ] 1.1 修改 `Cargo.toml` 第 60 行：将 `default = []` 改为 `default = ["gpu"]` [D1]
- [ ] 1.2 修改 `Cargo.toml` 第 55 行：为 wgpu 添加 vulkan feature，改为 `wgpu = { version = "24", optional = true, features = ["vulkan"] }` [D2]

## 2. GPU 初始化健壮性

- [ ] 2.1 在 `src/gpu/context.rs` 的 `try_new_sync()` 中实现 3 秒超时机制：将 `pollster::block_on` 移到 `thread::spawn` 子线程，主线程用 `mpsc::recv_timeout(Duration::from_secs(3))` 等待，超时记录 `warn` 日志（含 "timeout" 和 "3"）并返回 `unavailable()` [D3, GPU-TIMEOUT-FALLBACK-DISPLAY]
- [ ] 2.2 修改 `src/gpu/context.rs` 的 `init_gpu()` 中 `InstanceDescriptor`：按平台限制单一后端（Windows=DX12, macOS=METAL, Linux=VULKAN） [D6]

## 2.5 GPU 编码器防御性检查

- [ ] 2.5.1 在 `src/gpu/jpeg.rs` 的 `GpuJpegEncoder::encode()` 入口添加零尺寸防御检查：`if width == 0 || height == 0 { bail!("image has zero dimension") }`，防止 `pad_to_blocks` 中 u32 下溢和 `downsample_420` 中除零 [C2, R6]

## 3. GpuAccelerator 接口统一

- [ ] 3.1 在 `src/gpu/context.rs` 的 `#[cfg(not(feature = "gpu"))]` impl 块中补充 `name()` 方法（返回 `""`） [D5, GPU-STATUS-DATA-BINDING]
- [ ] 3.2 **[依赖 3.1]** 移除 `src/main.rs` 中 `set_gpu_name` 调用的 `#[cfg(feature = "gpu")]` 守卫，使其无条件执行。**必须在 3.1 完成后执行**，否则 `--no-default-features` 编译将因 `name()` 方法不存在而失败 [D5, GPU-STATUS-DATA-BINDING]

## 4. JPEG 编码策略修改

- [ ] 4.1 修改 `src/gpu/mod.rs` 的 `create_jpeg_encoder`：即使 GPU 可用也默认返回 `MozjpegEncoder`，保留 GPU JPEG 编码器代码供未来"速度优先"模式使用。同时更新 `create_jpeg_encoder` 和 `create_encoder` 的文档注释（doc comment），说明 JPEG 编码默认使用 mozjpeg 以保证压缩质量，GPU 加速仅用于预处理阶段（resize、色彩转换），GPU JPEG 编码器保留为未来"速度优先"模式的可选项 [D4]

## 5. UI Footer 文案更新

- [ ] 5.1 修改 `ui/pages/main.slint` Footer 的 GPU 不可用文案：从 `"GPU: 未启用 (需 --features gpu 编译)"` 改为 `"GPU: " + @tr("不可用 (CPU 模式)")`，可用分支保持 `"GPU: " + AppState.gpu-name` [D7, GPU-UNAVAILABLE-RUNTIME-DISPLAY, GPU-STATUS-I18N]
- [ ] 5.2 更新 i18n 翻译文件，添加 GPU 状态翻译 key `gpu_unavailable`：en=`"Unavailable (CPU mode)"`、zh-CN=`"不可用 (CPU 模式)"`、zh-TW=`"不可用 (CPU 模式)"`、ja=`"利用不可 (CPU モード)"`。注意 Slint @tr() 使用 gettext .po 格式，需确认当前项目 i18n 是否已有 .po 文件 [GPU-STATUS-I18N]

## 6. 验证与文档

- [ ] 6.1 编译验证：`cargo build`（默认 GPU）、`cargo build --no-default-features`（纯 CPU）、`cargo clippy --all-features -- -D warnings`、`cargo test` 全部通过
- [ ] 6.2 运行时验证（有 GPU）：Footer 显示适配器名称，JPEG 使用 mozjpeg，resize 使用 GPU
- [ ] 6.3 纯 CPU 模式验证：`cargo run --no-default-features`，Footer 显示 "GPU: 不可用 (CPU 模式)"，压缩功能正常
- [ ] 6.4 更新 CLAUDE.md 中以下章节：(a) Cargo.toml 依赖配置：`default = ["gpu"]`、wgpu 添加 `features = ["vulkan"]`；(b) 构建与运行：默认构建已包含 GPU，移除 `cargo build --release --features gpu` 说明，添加 `--no-default-features` 纯 CPU 构建说明；(c) 关键设计决策表：更新 GPU 层 feature 策略；(d) GPU 加速层说明：补充 JPEG 编码默认使用 mozjpeg、GPU 仅用于预处理的策略变更

## P2. 后续迭代（本次不做）

- [ ] P2.1 GPU 运行时故障恢复：注册 device lost callback，`available` 改为 `AtomicBool`
- [ ] P2.2 `GpuColorConverter` 超大图 CPU 回退
- [ ] P2.3 `GpuDct` buffer 大小检查与分块策略
- [ ] P2.4 零尺寸图像边界条件校验（Pipeline 入口统一校验）—— GpuJpegEncoder 入口已在 2.5 添加最小防御
- [ ] P2.5 多 GPU 适配器选择 UI
- [ ] P2.6 运行时 GPU 开关配置项
- [ ] P2.7 "速度优先"模式（GPU JPEG 编码器可选启用）
