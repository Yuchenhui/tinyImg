use crate::engine::codec::{Codec, Decoder};
use crate::engine::params::ImageFormat;
use crate::engine::raw_image::RawImage;
use anyhow::{Context, Result};
use std::io::Cursor;
use std::path::PathBuf;

/// 基于 image-rs 的通用解码器
///
/// 支持 JPEG / PNG / WebP / GIF / BMP / TIFF / ICO 等 image-rs 内置格式。
/// AVIF / JXL / SVG 等格式需要各自的专用解码器。
pub struct UniversalDecoder;

impl Codec for UniversalDecoder {
    fn name(&self) -> &'static str {
        "image-rs"
    }

    fn formats(&self) -> &[ImageFormat] {
        &[
            ImageFormat::Jpeg,
            ImageFormat::Png,
            ImageFormat::WebP,
            ImageFormat::Gif,
        ]
    }
}

impl Decoder for UniversalDecoder {
    fn decode(&self, data: &[u8], source_format: ImageFormat) -> Result<RawImage> {
        let cursor = Cursor::new(data);

        let image_format = match source_format {
            ImageFormat::Jpeg => image::ImageFormat::Jpeg,
            ImageFormat::Png => image::ImageFormat::Png,
            ImageFormat::WebP => image::ImageFormat::WebP,
            ImageFormat::Gif => image::ImageFormat::Gif,
            ImageFormat::Avif => image::ImageFormat::Avif,
            _ => {
                // 尝试自动探测
                let guessed = image::guess_format(data)
                    .context("Unable to guess image format")?;
                guessed
            }
        };

        let pixels = image::load(cursor, image_format)
            .with_context(|| format!("Failed to decode {source_format} image"))?;

        Ok(RawImage::new(pixels, source_format, PathBuf::new()))
    }
}
