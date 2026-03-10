pub mod avif;
pub mod gif;
pub mod jpeg;
pub mod png;
pub mod universal;
pub mod webp;

use crate::engine::params::{EncodeParams, ImageFormat};
use crate::engine::raw_image::RawImage;
use anyhow::Result;

/// 解码器：原始字节 → RawImage
pub trait Decoder: Send + Sync + 'static {
    fn decode(&self, data: &[u8], source_format: ImageFormat) -> Result<RawImage>;
}

/// 编码器：RawImage → 压缩后字节
pub trait Encoder: Send + Sync + 'static {
    fn encode(&self, image: &RawImage, params: &EncodeParams) -> Result<EncodedOutput>;
}

/// 编码输出
pub struct EncodedOutput {
    pub data: Vec<u8>,
}
