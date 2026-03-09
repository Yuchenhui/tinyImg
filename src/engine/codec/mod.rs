pub mod avif;
pub mod gif;
pub mod jpeg;
pub mod jxl;
pub mod png;
pub mod svg;
pub mod universal;
pub mod webp;

use crate::engine::params::{EncodeParams, ImageFormat};
use crate::engine::raw_image::RawImage;
use anyhow::Result;

/// 编解码器基础信息
pub trait Codec: Send + Sync + 'static {
    /// 编解码器名称
    fn name(&self) -> &'static str;
    /// 支持的格式列表
    fn formats(&self) -> &[ImageFormat];
}

/// 解码器：原始字节 → RawImage
pub trait Decoder: Codec {
    fn decode(&self, data: &[u8], source_format: ImageFormat) -> Result<RawImage>;
}

/// 编码器：RawImage → 压缩后字节
pub trait Encoder: Codec {
    fn encode(&self, image: &RawImage, params: &EncodeParams) -> Result<EncodedOutput>;
}

/// 编码输出
pub struct EncodedOutput {
    pub data: Vec<u8>,
    pub format: ImageFormat,
}
