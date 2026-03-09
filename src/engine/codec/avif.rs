use crate::engine::codec::{Codec, EncodedOutput, Encoder};
use crate::engine::params::{EncodeParams, ImageFormat};
use crate::engine::raw_image::RawImage;
use anyhow::{bail, Context, Result};
use ravif::RGBA8;

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

        let rgba = image.pixels.to_rgba8();
        let (width, height) = rgba.dimensions();

        // 将 image-rs RGBA 数据转换为 ravif 的 RGBA8
        let pixels: Vec<RGBA8> = rgba
            .pixels()
            .map(|p| RGBA8::new(p[0], p[1], p[2], p[3]))
            .collect();

        let img = ravif::Img::new(&pixels[..], width as usize, height as usize);

        let enc = ravif::Encoder::new()
            .with_quality(*quality as f32)
            .with_speed(*speed)
            .with_alpha_quality(*quality as f32);

        let result = enc.encode_rgba(img).context("AVIF encoding failed")?;

        Ok(EncodedOutput {
            data: result.avif_file,
            format: ImageFormat::Avif,
        })
    }
}
