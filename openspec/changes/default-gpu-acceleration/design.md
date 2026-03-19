## Context

TinyImg 的 GPU 加速模块（wgpu compute shader）已完整实现，包括：
- GPU 缩放（`src/gpu/resize.rs`）—— 双线性插值 compute shader
- GPU JPEG 编码（`src/gpu/jpeg.rs`）—— 自建 DCT+量化+Zigzag GPU 管线 + CPU Huffman/JFIF
- GPU 色彩转换（`src/gpu/color.rs`）—— RGB↔YCbCr compute shader
- GPU DCT（`src/gpu/dct.rs`）—— 独立 8x8 块 DCT 变换

当前 `gpu` feature 为 optional（`default = []`），用户需手动 `--features gpu` 编译。GPU 上下文在 `App::new()` 中同步初始化，失败时静默降级到 CPU。`AppState` 中的 `gpu-available` 和 `gpu-name` 属性已定义，UI Footer 中已有 GPU 状态展示。

## Goals / Non-Goals

**Goals:**

- 将 `gpu` feature 设为默认启用，让有 GPU 的用户开箱即用获得加速
- 保持 GPU 不可用时的无缝 CPU 降级体验
- 在 UI 状态栏正确展示 GPU 加速状态
- 解决默认启用后暴露的健壮性问题（超时保护、运行时故障恢复）

**Non-Goals:**

- 不改为异步 GPU 初始化（架构改动大，当前同步+超时方案足够）
- 不实现多 GPU 适配器选择 UI（后续迭代）
- 不实现运行时 GPU 开关配置项（后续迭代，当前靠编译时 feature 控制）
- 不新增 `cpu-only` 或 `no-gpu` feature（`--no-default-features` 已满足）
- 不实现 GPU 性能对比展示

## Decisions

### D1: Feature 配置 —— `default = ["gpu"]`，不引入额外 feature

将 `Cargo.toml` 中 `default` 从 `[]` 改为 `["gpu"]`。不新增 `cpu-only`/`no-gpu` feature。

**理由**: TinyImg 是 binary crate，不存在下游库使用者需要精细控制 feature 的场景。Cargo 原生的 `--no-default-features` 已满足纯 CPU 构建需求。引入冗余 feature 会增加条件编译矩阵和维护负担。

**备选方案（否决）**:
- 方案 B: 新增 `cpu-only` feature —— 与 `--no-default-features` 语义重复
- 方案 C: 新增 `no-gpu` feature —— 双重否定逻辑，可读性差

### D2: 修复 wgpu v24 的 Vulkan feature 缺失

**关键发现**: wgpu v24 的默认 features 为 `wgsl, dx12, metal, webgpu`，**不包含 `vulkan`**（从 v25 起才默认包含）。当前 `Cargo.toml` 中 `wgpu = { version = "24", optional = true }` 未显式启用 vulkan，**导致 Linux 上没有任何可用后端**。

**方案**: 显式添加 vulkan feature：
```toml
wgpu = { version = "24", optional = true, features = ["vulkan"] }
```

**与 D6 的交互说明**: D6 按平台限制后端（Windows=DX12, macOS=METAL, Linux=VULKAN），这意味着非 Linux 平台在运行时不会使用 Vulkan 后端。但 `features = ["vulkan"]` 仍需全平台启用，因为 Cargo features 是编译时全局的，无法按 `target_os` 条件启用 feature 依赖——必须确保 Linux 编译通过。后续优化可考虑在 `Cargo.toml` 中使用 `[target.'cfg(target_os = "linux")'.dependencies]` 将 Vulkan 相关的系统库依赖（如 `ash`）改为平台条件依赖，以减少非 Linux 平台的编译开销，但这需要评估 wgpu 内部 feature 结构的可行性。

**备选方案（否决）**:
- 升级到 wgpu v25+ —— 变更范围扩大，API 可能有 breaking changes，应在独立变更中处理

### D3: GPU 初始化增加 3 秒超时保护

当前 `GpuAccelerator::try_new_sync()` 使用 `pollster::block_on` 同步等待，某些驱动（如 Windows 老旧 Intel iGPU、AMD Vulkan）可能导致 `request_adapter` 或 `request_device` 长时间阻塞甚至卡死。

**方案**: 在 `try_new_sync()` 中将 GPU 初始化移到独立线程，主线程用 `mpsc::recv_timeout(Duration::from_secs(3))` 等待结果。超时则返回 `GpuAccelerator::unavailable()`。

```
伪逻辑：
thread::spawn → pollster::block_on(try_new()) → tx.send(result)
主线程: rx.recv_timeout(3s) → Ok(acc) 或 Timeout → unavailable()
```

**理由**: 正常 GPU 初始化不超过 500ms，3 秒给足裕量。超时后线程继续运行也无害（不持有主线程引用，进程退出自动清理）。比改为异步初始化（需 Arc<RwLock<GpuAccelerator>> + invoke_from_event_loop 复杂交互）简单得多。

**超时后线程资源清理**: 超时后子线程可能仍在执行 GPU 初始化。如果子线程最终成功创建了 `wgpu::Device` 和 `wgpu::Queue`，这些资源会在 `tx.send()` 失败（接收端已关闭）后随 `GpuAccelerator` 实例 drop 而释放——Rust 的 ownership 机制保证了这一点，无需额外清理。子线程本身在完成后自然终止。在极端情况下（如驱动 hang 导致子线程永不返回），该线程会被操作系统在进程退出时回收。

**注意**: `src/app.rs` 第 20 行附近存在描述 GPU 初始化行为的注释（如「同步初始化，失败时静默降级」），实施本决策后需同步更新该注释以反映超时机制的引入（即「同步初始化，带 3 秒超时保护，超时或失败时静默降级到 CPU」）。

**超时时长 3 秒的依据**:
- 社区报告：正常系统 200-500ms，问题系统可达 10+ 秒
- 3 秒是桌面应用启动可接受上限
- wezterm 也采用类似的超时策略

### D4: GPU JPEG 编码策略 —— 仅加速预处理，JPEG 编码默认仍用 mozjpeg

**关键发现**: `src/gpu/jpeg.rs` 中的 GPU JPEG 编码器是自建实现（标准量化表 + 标准 Huffman 表），没有 mozjpeg 的率失真优化、自适应量化、trellis 量化等高级特性。同等 quality 值下，GPU JPEG 输出文件体积比 mozjpeg 大 10-30%。

默认启用 GPU 后，`create_jpeg_encoder` 会静默将 mozjpeg 替换为 GPU JPEG 编码器，**这是一个隐性的质量降级**。

**方案**: 修改 `create_jpeg_encoder` 的默认行为——**即使 GPU 可用，JPEG 编码仍默认使用 mozjpeg**。GPU 加速仅应用于预处理阶段（resize、色彩转换），这些操作 GPU/CPU 输出质量等价，纯粹是加速。

GPU JPEG 编码器保留，作为未来"速度优先"模式的选项（需要用户显式选择）。

**理由**:
- 图片压缩工具的核心价值是压缩质量，不应因加速而牺牲
- resize/色彩转换是像素级操作，GPU 和 CPU 结果数值等价（允许浮点误差）
- JPEG 编码涉及量化策略差异，GPU 自建实现无法与 mozjpeg 20+ 年的优化竞争

### D5: 统一 `GpuAccelerator` 公开接口，消除业务代码中的条件编译守卫

当前 `#[cfg(not(feature = "gpu"))]` 的 `GpuAccelerator` impl 只有 `try_new_sync()`，缺少 `name()` 方法。导致 `main.rs` 中 `set_gpu_name` 调用被 `#[cfg(feature = "gpu")]` 包裹。

**方案**: 在 `#[cfg(not(feature = "gpu"))]` impl 中补充 `pub fn name(&self) -> &str { "" }` 等缺失方法，使业务代码（如 `main.rs`、`app.rs`）无需条件编译守卫即可统一调用 `GpuAccelerator` 的方法。注意：本决策的 scope 限于消除业务代码中的 `#[cfg]` 守卫，不涉及 `GpuAccelerator` 内部实现的条件编译（那些是必要的）。

### D6: 按平台限制 wgpu 后端，减少初始化风险

当前 `init_gpu()` 同时请求 `VULKAN | DX12 | METAL` 三个后端。多后端初始化增加耗时和崩溃风险（尤其 Windows 上 DX12+Vulkan 同时初始化可达 500ms+）。

**方案**: 按目标平台仅启用对应后端：
- Windows: `DX12`（DX12 在 Windows 上覆盖最广、最稳定）
- macOS: `METAL`
- Linux: `VULKAN`

**理由**: 桌面应用不需要跨后端回退（不同于 WebGPU 场景），单后端足够。社区报告确认限制后端数量能显著减少初始化时间和驱动兼容性问题。

### D7: 更新 UI Footer 文案

当前 `main.slint` Footer 在无 GPU 时显示 `"GPU: 未启用 (需 --features gpu 编译)"`。默认启用后此提示不再准确。

**方案**: 改为 `"GPU: 不可用 (CPU 模式)"`。

## Risks / Trade-offs

### R1: 驱动 segfault 无法捕获 → 进程崩溃
`catch_unwind` 只能捕获 Rust panic，无法捕获驱动层 SIGSEGV/ACCESS_VIOLATION。已知 Windows 老旧 Intel iGPU 驱动存在此问题（约 5-10% Windows 用户）。

**缓解**: D6（限制单后端）减少触发概率；中期关注 wgpu 驱动黑名单功能（Issue #7241）；长期可考虑子进程探测方案。

### R2: 编译体积增加 ~5-10MB，首次编译增加 60-120 秒
**接受**: 对桌面应用体积可接受；用户通常使用预编译二进制；增量编译影响有限。

### R3: GPU 运行时故障（TDR/Device Lost）未恢复
当前设计为"初始化时降级"，GPU 初始化成功后中途失败（如 Windows TDR 超时）会导致后续所有任务失败。

**缓解**: 本次变更范围内暂不实现运行时降级（改动面大），但需在 tasks 中记录为 P2 后续项。建议注册 wgpu device lost callback + 将 `available` 改为 `AtomicBool`。

> **注意**: P2.1（运行时故障恢复）将 `available` 改为 `AtomicBool` 会修改 `GpuAccelerator` 的公共接口，这可能与现有 gpu-acceleration spec 的 immutability 约束产生冲突。实施 P2.1 时需评估是否需要为此创建新的 spec 变更，或判定其为既有 spec 的 bug fix 范畴。此为已知的未来冲突点，记录于此以便后续决策。

### R4: 质量风险 —— GpuColorConverter 和 GpuDct 缺少降级路径
quality-engineer 发现 `GpuColorConverter` 超大图直接 bail 无 CPU 回退；`GpuDct` 无 buffer 大小检查。

**缓解**: 当前主流程中 GpuColorConverter 和 GpuDct 未被直接调用（JPEG 管线内联了自己的 DCT shader），风险暂不暴露。但作为 pub 模块存在隐患，tasks 中标记为需修复项。

### R5: device.poll(Maintain::Wait) 在 rayon 并行中的性能陷阱
多个 rayon 线程共享同一 device，一个线程的 `poll(Wait)` 会等待所有已提交命令（包括其他线程的）完成。批量处理时 GPU 利用率可能不如预期。

**接受**: 本次变更不改动并行模型。D4 决策（JPEG 仍用 mozjpeg）已大幅减少 GPU 在并行场景中的使用面。GPU resize 仍有此风险，但缩放操作耗时短，影响有限。

### R6: 零尺寸图像边界条件
`jpeg.rs` 中 `pad_to_blocks` 对 width=0 会触发 u32 下溢；`downsample_420` 对 width=0 或 height=0 会除零。

**缓解**: 应在管线入口处（Pipeline::run）统一校验输入尺寸，但这超出本次变更范围。tasks 中标记为需修复项。
