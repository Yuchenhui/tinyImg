use std::sync::Arc;

/// GPU 加速器上下文
///
/// 管理 wgpu device/queue 生命周期。
/// 启动时异步探测 GPU，失败则标记为不可用，全程使用 CPU fallback。
pub struct GpuAccelerator {
    #[cfg(feature = "gpu")]
    pub device: Option<Arc<wgpu::Device>>,
    #[cfg(feature = "gpu")]
    pub queue: Option<Arc<wgpu::Queue>>,
    #[cfg(feature = "gpu")]
    pub adapter_name: String,
    available: bool,
}

#[cfg(feature = "gpu")]
impl GpuAccelerator {
    /// 尝试初始化 GPU（不阻塞，失败返回不可用实例）
    pub async fn try_new() -> Self {
        match Self::init_gpu().await {
            Ok((device, queue, adapter_name)) => {
                tracing::info!("GPU acceleration enabled: {adapter_name}");
                Self {
                    device: Some(Arc::new(device)),
                    queue: Some(Arc::new(queue)),
                    adapter_name,
                    available: true,
                }
            }
            Err(e) => {
                tracing::warn!("GPU not available, falling back to CPU: {e}");
                Self::unavailable()
            }
        }
    }

    /// 同步版初始化（在当前线程 block 执行）
    ///
    /// 如果 GPU 初始化过程中发生崩溃（如驱动问题），
    /// 会捕获 panic 并安全降级到 CPU 模式。
    pub fn try_new_sync() -> Self {
        match std::panic::catch_unwind(|| pollster::block_on(Self::try_new())) {
            Ok(acc) => acc,
            Err(e) => {
                let msg = if let Some(s) = e.downcast_ref::<&str>() {
                    s.to_string()
                } else if let Some(s) = e.downcast_ref::<String>() {
                    s.clone()
                } else {
                    "unknown panic".to_string()
                };
                tracing::error!("GPU initialization panicked, falling back to CPU: {msg}");
                Self::unavailable()
            }
        }
    }

    async fn init_gpu() -> anyhow::Result<(wgpu::Device, wgpu::Queue, String)> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::VULKAN | wgpu::Backends::DX12 | wgpu::Backends::METAL,
            ..Default::default()
        });
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                ..Default::default()
            })
            .await
            .ok_or_else(|| anyhow::anyhow!("No suitable GPU adapter found"))?;

        let adapter_name = adapter.get_info().name.clone();
        tracing::info!("GPU adapter: {adapter_name}");

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor::default(), None)
            .await?;

        Ok((device, queue, adapter_name))
    }

    /// 创建一个不可用的 GPU 实例（CPU-only fallback）
    pub fn unavailable() -> Self {
        Self {
            device: None,
            queue: None,
            adapter_name: String::new(),
            available: false,
        }
    }

    /// 获取 device 引用（仅在 GPU 可用时有效）
    pub fn device(&self) -> Option<&wgpu::Device> {
        self.device.as_deref()
    }

    /// 获取 queue 引用
    pub fn queue(&self) -> Option<&wgpu::Queue> {
        self.queue.as_deref()
    }

    /// GPU 适配器名称
    pub fn name(&self) -> &str {
        &self.adapter_name
    }
}

#[cfg(not(feature = "gpu"))]
impl GpuAccelerator {
    pub fn try_new_sync() -> Self {
        Self { available: false }
    }

    pub fn unavailable() -> Self {
        Self { available: false }
    }
}

impl GpuAccelerator {
    /// GPU 是否可用
    pub fn is_available(&self) -> bool {
        self.available
    }

    /// 是否支持 compute shader
    pub fn supports_compute(&self) -> bool {
        self.available
    }
}
