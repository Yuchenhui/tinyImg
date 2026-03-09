pub mod progress;
pub mod task;

use crate::worker::progress::ProgressUpdate;
use crate::worker::task::{CompressionTask, TaskStatus};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// 任务管理器
///
/// 负责接收文件列表、在后台线程中启动 rayon 并行处理、
/// 通过回调向 UI 报告进度、支持取消操作。
pub struct TaskManager {
    /// 取消标志
    cancel_flag: Arc<AtomicBool>,
    /// 下一个任务 ID
    next_id: u64,
}

impl TaskManager {
    pub fn new() -> Self {
        Self {
            cancel_flag: Arc::new(AtomicBool::new(false)),
            next_id: 0,
        }
    }

    /// 分配新的任务 ID
    pub fn next_task_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    /// 请求取消当前批次
    pub fn cancel(&self) {
        self.cancel_flag.store(true, Ordering::Relaxed);
    }

    /// 检查是否已取消
    pub fn is_cancelled(&self) -> bool {
        self.cancel_flag.load(Ordering::Relaxed)
    }

    /// 重置取消标志（开始新批次前调用）
    pub fn reset(&self) {
        self.cancel_flag.store(false, Ordering::Relaxed);
    }

    /// 获取取消标志的克隆（传入工作线程）
    pub fn cancel_token(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.cancel_flag)
    }
}

impl Default for TaskManager {
    fn default() -> Self {
        Self::new()
    }
}
