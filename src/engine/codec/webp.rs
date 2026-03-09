use crate::engine::codec::{Codec, EncodedOutput, Encoder};
use crate::engine::params::{EncodeParams, ImageFormat};
use crate::engine::raw_image::RawImage;
use anyhow::{bail, Context, Result};

/// image-webp 纯 Rust WebP 编码器
///
/// 注意：当前 image-webp 仅支持无损 VP8L 编码。
/// 有损 WebP 编码需要 libwebp C 绑定（未来可添加）。
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
        let EncodeParams::WebP { quality: _, lossless: _ } = params else {
            bail!("WebpEncoder requires WebP params");
        };

        // image-webp 目前仅支持无损 VP8L
        // quality 参数暂不影响编码（无损模式无质量概念）
        let rgba = image.pixels.to_rgba8();
        let (width, height) = rgba.dimensions();

        let mut buf = Vec::new();
        let encoder = image_webp::WebPEncoder::new(&mut buf);
        encoder
            .encode(rgba.as_raw(), width, height, image_webp::ColorType::Rgba8)
            .context("WebP encoding failed")?;

        Ok(EncodedOutput {
            data: buf,
            format: ImageFormat::WebP,
        })
    }
}
