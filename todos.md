# TinyImg 待办清单

## 高优先级

- [x] 注册预设相关 bridge callback（`select-preset`、`save-custom-preset`、`delete-preset`）
- [x] UI 质量参数可配置（连接 slider/input 到 `EncodeParams`，替换硬编码默认值）
- [x] 输出配置生效（输出目录、文件后缀、是否覆盖，读取 `AppConfig` 而非硬编码 `_compressed`）

## 中优先级

- [x] 格式转换 UI（用户可选择输出格式，支持 JPEG/PNG/WebP/AVIF/GIF 互转）
- [ ] JXL 编码器实现（jxl-oxide 仅解码，纯 Rust 无可用编码器；JXL 输入自动降级为 PNG 输出）
- [ ] SVG 优化器集成（需 svgo sidecar 或 WASM）
- [x] WebP 有损编码支持（添加 webp crate，libwebp 提供有损 VP8 + 质量控制）

## 低优先级

- [x] Resize 预处理接入（已连接 UI 设置，自动选择 GPU/CPU）
- [x] 元数据剥离可配置（设置页 CheckBox 控制）
- [ ] 压缩前后预览对比（需生成缩略图 + UI 比较组件）
- [x] 单张压缩回调（`compress-single` callback 已实现）
- [x] 设置页回调注册（语言切换、主题切换、输出目录浏览等）

## 性能优化

- [ ] 评估 zenrav1e 替代 rav1e 提升 AVIF 编码速度
- [ ] GPU JPEG 编码器 progressive 模式支持（当前仅 baseline）
- [ ] 大图分块 GPU 处理（避免超出 `max_storage_buffer_binding_size` 128MB 限制）

## 已完成

- [x] 7 种格式解码器（JPEG/PNG/WebP/AVIF/JXL/GIF/TIFF）
- [x] 5 种格式编码器（JPEG/PNG/WebP/AVIF/GIF）
- [x] 压缩管线（decode → preprocess → encode）
- [x] 批量并行压缩（rayon）
- [x] GPU 加速缩放（wgpu compute shader）
- [x] GPU 加速 JPEG 编码（DCT+量化 on GPU，2.3-7.3x 提速）
- [x] 文件拖拽/选择 UI
- [x] 压缩进度反馈到 UI
- [x] GPU 自动探测与降级
- [x] 管线缓存（OnceLock 避免重复编译 shader）
- [x] 预设管理（4 个内置预设 + 自定义预设保存/删除）
- [x] 完整设置页（格式参数/输出配置/处理设置/GPU 状态）
- [x] 配置持久化（TOML 自动保存/加载）
- [x] 格式转换（保持原格式 / 指定输出格式）
- [x] WebP 有损编码（libwebp VP8）
- [x] JXL/SVG 输入自动降级为 PNG 输出
- [x] 单张压缩功能
