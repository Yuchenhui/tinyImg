pub mod metadata;
pub mod resize;

use crate::engine::raw_image::RawImage;
use anyhow::Result;

/// 预处理步骤 trait
///
/// Pipeline 中按序执行，每个预处理器接收 RawImage 并返回处理后的 RawImage。
/// GPU 实现和 CPU 实现共享此 trait，可透明互换。
pub trait Preprocessor: Send + Sync {
    /// 预处理器名称
    fn name(&self) -> &'static str;
    /// 执行预处理
    fn process(&self, image: RawImage) -> Result<RawImage>;
}
