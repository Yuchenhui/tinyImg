use crate::engine::codec::{Codec, Encoder, EncodedOutput};
use crate::engine::params::{EncodeParams, ImageFormat};
use crate::engine::raw_image::RawImage;
use anyhow::{bail, Result};

/// image-webp 纯 Rust WebP 编码器
pub struct WebpEncoder;

impl Codec for WebpEncoder {
    fn name(&self) -> &'static str {
        "image-webp"
    }

    fn formats(&self) -> &[ImageFormat] {
        &[ImageFormat::WebP]
    }
}

impl Encoder for WebpEncoder {
    fn encode(&self, image: &RawImage, params: &EncodeParams) -> Result<EncodedOutput> {
        let EncodeParams::WebP { quality, lossless } = params else {
            bail!("WebpEncoder requires WebP params");
        };

        // TODO: 实现 image-webp 编码
        let _ = (image, quality, lossless);
        todo!("Implement WebP encoding")
    }
}
