use crate::engine::codec::{Codec, Encoder, EncodedOutput};
use crate::engine::params::{EncodeParams, ImageFormat};
use crate::engine::raw_image::RawImage;
use anyhow::{bail, Result};

/// ravif 纯 Rust AVIF 编码器
pub struct AvifEncoder;

impl Codec for AvifEncoder {
    fn name(&self) -> &'static str {
        "ravif"
    }

    fn formats(&self) -> &[ImageFormat] {
        &[ImageFormat::Avif]
    }
}

impl Encoder for AvifEncoder {
    fn encode(&self, image: &RawImage, params: &EncodeParams) -> Result<EncodedOutput> {
        let EncodeParams::Avif { quality, speed } = params else {
            bail!("AvifEncoder requires Avif params");
        };

        // TODO: 实现 ravif 编码
        let _ = (image, quality, speed);
        todo!("Implement AVIF encoding")
    }
}
