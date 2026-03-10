use crate::engine::codec::{EncodedOutput, Encoder};
use crate::engine::params::EncodeParams;
use crate::engine::raw_image::RawImage;
use anyhow::{bail, Result};
use image::DynamicImage;

/// mozjpeg-rs 纯 Rust JPEG 编码器
pub struct MozjpegEncoder;

impl Encoder for MozjpegEncoder {
    fn encode(&self, image: &RawImage, params: &EncodeParams) -> Result<EncodedOutput> {
        let EncodeParams::Jpeg {
            quality,
            progressive,
        } = params
        else {
            bail!("MozjpegEncoder requires Jpeg params");
        };

        let preset = if *progressive {
            mozjpeg_rs::Preset::ProgressiveBalanced
        } else {
            mozjpeg_rs::Preset::BaselineBalanced
        };

        let data = match &image.pixels {
            DynamicImage::ImageLuma8(gray) => {
                let (w, h) = gray.dimensions();
                mozjpeg_rs::Encoder::new(preset)
                    .quality(*quality)
                    .encode_gray(gray.as_raw(), w, h)?
            }
            _ => {
                // 转换为 RGB8 后编码
                let rgb = image.pixels.to_rgb8();
                let (w, h) = rgb.dimensions();
                mozjpeg_rs::Encoder::new(preset)
                    .quality(*quality)
                    .encode_rgb(rgb.as_raw(), w, h)?
            }
        };

        Ok(EncodedOutput {
            data: data.to_vec(),
        })
    }
}
