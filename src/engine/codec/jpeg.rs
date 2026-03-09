use crate::engine::codec::{Codec, Encoder, EncodedOutput};
use crate::engine::params::{EncodeParams, ImageFormat};
use crate::engine::raw_image::RawImage;
use anyhow::{bail, Result};

/// mozjpeg-rs 纯 Rust JPEG 编码器
pub struct MozjpegEncoder;

impl Codec for MozjpegEncoder {
    fn name(&self) -> &'static str {
        "mozjpeg-rs"
    }

    fn formats(&self) -> &[ImageFormat] {
        &[ImageFormat::Jpeg]
    }
}

impl Encoder for MozjpegEncoder {
    fn encode(&self, image: &RawImage, params: &EncodeParams) -> Result<EncodedOutput> {
        let EncodeParams::Jpeg {
            quality,
            progressive,
        } = params
        else {
            bail!("MozjpegEncoder requires Jpeg params");
        };

        // TODO: 实现 mozjpeg-rs 编码
        let _ = (image, quality, progressive);
        todo!("Implement mozjpeg-rs encoding")
    }
}
