use std::path::PathBuf;

/// 压缩任务
pub struct CompressionTask {
    pub id: u64,
    pub input_path: PathBuf,
    pub output_path: PathBuf,
}

/// 任务状态机
///
/// ```text
/// Pending → Processing → Completed { original_size, compressed_size }
///                      └→ Failed { error }
/// ```
#[derive(Debug, Clone)]
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
    /// 获取压缩率（0.0 ~ 1.0）
    pub fn compression_ratio(&self) -> Option<f64> {
        match self {
            Self::Completed {
                original_size,
                compressed_size,
            } => {
                if *original_size == 0 {
                    return Some(0.0);
                }
                Some(1.0 - (*compressed_size as f64 / *original_size as f64))
            }
            _ => None,
        }
    }

    pub fn is_completed(&self) -> bool {
        matches!(self, Self::Completed { .. })
    }

    pub fn is_failed(&self) -> bool {
        matches!(self, Self::Failed { .. })
    }
}
