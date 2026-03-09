/// GPU 加速器上下文
///
/// 管理 wgpu device/queue 生命周期。
/// 启动时异步探测 GPU，失败则标记为不可用，全程使用 CPU fallback。
pub struct GpuAccelerator {
    #[cfg(feature = "gpu")]
    pub device: std::sync::Arc<wgpu::Device>,
    #[cfg(feature = "gpu")]
    pub queue: std::sync::Arc<wgpu::Queue>,
    available: bool,
}

impl GpuAccelerator {
    /// 尝试初始化 GPU（不阻塞，失败返回不可用实例）
    #[cfg(feature = "gpu")]
    pub async fn try_new() -> Self {
        match Self::init_gpu().await {
            Ok((device, queue)) => {
                tracing::info!("GPU acceleration enabled");
                Self {
                    device: std::sync::Arc::new(device),
                    queue: std::sync::Arc::new(queue),
                    available: true,
                }
            }
            Err(e) => {
                tracing::warn!("GPU not available, falling back to CPU: {e}");
                Self::unavailable()
            }
        }
    }

    #[cfg(feature = "gpu")]
    async fn init_gpu() -> anyhow::Result<(wgpu::Device, wgpu::Queue)> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                ..Default::default()
            })
            .await
            .ok_or_else(|| anyhow::anyhow!("No suitable GPU adapter found"))?;

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor::default())
            .await?;

        Ok((device, queue))
    }

    /// 创建一个不可用的 GPU 实例（CPU-only fallback）
    pub fn unavailable() -> Self {
        Self {
            #[cfg(feature = "gpu")]
            device: {
                // 这个分支不会被调用到——仅在 try_new 失败时使用
                unreachable!()
            },
            #[cfg(feature = "gpu")]
            queue: {
                unreachable!()
            },
            available: false,
        }
    }

    /// GPU 是否可用
    pub fn is_available(&self) -> bool {
        self.available
    }

    /// 是否支持 compute shader
    pub fn supports_compute(&self) -> bool {
        self.available
    }
}

#[cfg(not(feature = "gpu"))]
impl GpuAccelerator {
    pub fn try_new_sync() -> Self {
        Self { available: false }
    }
}
