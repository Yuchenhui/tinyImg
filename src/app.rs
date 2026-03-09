use crate::config::storage::ConfigStorage;
use crate::config::AppConfig;
use crate::engine::registry::CodecRegistry;
use crate::gpu::context::GpuAccelerator;
use crate::worker::TaskManager;

/// 应用核心结构体
///
/// 持有压缩引擎、配置、GPU 加速器、任务管理器等核心状态，
/// 连接 Slint UI bridge 和后端逻辑。
pub struct App {
    pub config: AppConfig,
    pub registry: CodecRegistry,
    pub task_manager: TaskManager,
    pub gpu: GpuAccelerator,
}

impl App {
    pub fn new() -> anyhow::Result<Self> {
        let config = ConfigStorage::load().unwrap_or_else(|e| {
            tracing::warn!("Failed to load config, using defaults: {e}");
            AppConfig::default()
        });

        let registry = CodecRegistry::new();
        let task_manager = TaskManager::new();

        // 尝试初始化 GPU（同步，不阻塞太久——探测失败会快速返回）
        let gpu = GpuAccelerator::try_new_sync();

        Ok(Self {
            config,
            registry,
            task_manager,
            gpu,
        })
    }
}
