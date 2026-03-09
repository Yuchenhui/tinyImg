use crate::engine::codec::{Codec, EncodedOutput, Encoder};
use crate::engine::params::{EncodeParams, ImageFormat};
use crate::engine::raw_image::RawImage;
use anyhow::{bail, Context, Result};

/// WebP 编码器
///
/// 有损模式使用 libwebp（通过 webp crate），支持质量控制。
/// 无损模式使用 image-webp（纯 Rust VP8L）。
pub struct WebpEncoder;

impl Codec for WebpEncoder {
    fn name(&self) -> &'static str {
        "webp"
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

        let rgba = image.pixels.to_rgba8();
        let (width, height) = rgba.dimensions();

        let data = if *lossless {
            // 无损模式：使用 image-webp（纯 Rust VP8L）
            let mut buf = Vec::new();
            let encoder = image_webp::WebPEncoder::new(&mut buf);
            encoder
                .encode(rgba.as_raw(), width, height, image_webp::ColorType::Rgba8)
                .context("WebP lossless encoding failed")?;
            buf
        } else {
            // 有损模式：使用 libwebp
            let encoder = webp::Encoder::from_rgba(rgba.as_raw(), width, height);
            let mem = encoder.encode(*quality as f32);
            mem.to_vec()
        };

        Ok(EncodedOutput {
            data,
            format: ImageFormat::WebP,
        })
    }
}
