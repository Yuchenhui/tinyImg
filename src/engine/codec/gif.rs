use crate::engine::codec::{Codec, EncodedOutput, Encoder};
use crate::engine::params::{EncodeParams, ImageFormat};
use crate::engine::raw_image::RawImage;
use anyhow::{bail, Context, Result};
use std::io::Cursor;

/// GIF 编码器（基于 image-rs 内置编码器）
///
/// gifski 主要用于从帧序列创建高质量 GIF 动画。
/// 对于单帧 GIF 重编码，使用 image-rs 内置的 GIF 编码器即可。
pub struct GifEncoder;

impl Codec for GifEncoder {
    fn name(&self) -> &'static str {
        "gif"
    }

    fn formats(&self) -> &[ImageFormat] {
        &[ImageFormat::Gif]
    }
}

impl Encoder for GifEncoder {
    fn encode(&self, image: &RawImage, params: &EncodeParams) -> Result<EncodedOutput> {
        let EncodeParams::Gif {
            quality: _,
            fast: _,
        } = params
        else {
            bail!("GifEncoder requires Gif params");
        };

        // 使用 image-rs 的 GIF 编码
        let mut buf = Cursor::new(Vec::new());
        image
            .pixels
            .write_to(&mut buf, image::ImageFormat::Gif)
            .context("GIF encoding failed")?;

        Ok(EncodedOutput {
            data: buf.into_inner(),
            format: ImageFormat::Gif,
        })
    }
}
