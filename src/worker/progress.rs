use crate::worker::task::TaskStatus;

/// 进度更新消息（从工作线程发往 UI 线程）
#[derive(Debug, Clone)]
pub struct ProgressUpdate {
    /// 任务 ID
    pub task_id: u64,
    /// 新状态
    pub status: TaskStatus,
    /// 文件名（用于 UI 显示）
    pub filename: String,
}
