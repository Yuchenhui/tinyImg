use crate::engine::codec::{Codec, EncodedOutput, Encoder};
use crate::engine::params::{EncodeParams, ImageFormat};
use crate::engine::raw_image::RawImage;
use anyhow::{bail, Context, Result};
use std::io::Cursor;

/// oxipng 无损 PNG 优化器（可选 imagequant 有损量化）
pub struct OxipngEncoder;

impl Codec for OxipngEncoder {
    fn name(&self) -> &'static str {
        "oxipng"
    }

    fn formats(&self) -> &[ImageFormat] {
        &[ImageFormat::Png]
    }
}

impl Encoder for OxipngEncoder {
    fn encode(&self, image: &RawImage, params: &EncodeParams) -> Result<EncodedOutput> {
        let EncodeParams::Png {
            lossy,
            optimization_level,
        } = params
        else {
            bail!("OxipngEncoder requires Png params");
        };

        let png_data = if *lossy {
            encode_lossy_png(image, *optimization_level)?
        } else {
            encode_lossless_png(image, *optimization_level)?
        };

        Ok(EncodedOutput {
            data: png_data,
            format: ImageFormat::Png,
        })
    }
}

/// 无损路径：image-rs 编码 → oxipng 优化
fn encode_lossless_png(image: &RawImage, optimization_level: u8) -> Result<Vec<u8>> {
    // 先用 image-rs 编码为 PNG 字节
    let raw_png = encode_to_png_bytes(image)?;

    // 用 oxipng 优化
    let opts = oxipng_options(optimization_level);
    let optimized = oxipng::optimize_from_memory(&raw_png, &opts)
        .context("oxipng optimization failed")?;

    Ok(optimized)
}

/// 有损路径：imagequant 量化 → PNG 编码 → oxipng 优化
fn encode_lossy_png(image: &RawImage, optimization_level: u8) -> Result<Vec<u8>> {
    let rgba = image.pixels.to_rgba8();
    let (width, height) = rgba.dimensions();
    let pixels_raw = rgba.as_raw();

    // 将 &[u8] 转换为 &[imagequant::RGBA]
    let pixels: &[imagequant::RGBA] = unsafe {
        std::slice::from_raw_parts(
            pixels_raw.as_ptr() as *const imagequant::RGBA,
            (width * height) as usize,
        )
    };

    // imagequant 量化
    let mut attr = imagequant::new();
    attr.set_quality(0, 80).context("imagequant set_quality failed")?;

    let mut img = attr
        .new_image(pixels, width as usize, height as usize, 0.0)
        .context("imagequant new_image failed")?;

    let mut res = attr.quantize(&mut img).context("imagequant quantize failed")?;
    res.set_dithering_level(1.0).context("set_dithering failed")?;

    let (palette, indexed_pixels) = res.remapped(&mut img).context("imagequant remap failed")?;

    // 用 png crate 写入索引 PNG
    let mut png_data = Vec::new();
    {
        let mut encoder = png::Encoder::new(Cursor::new(&mut png_data), width, height);
        encoder.set_color(png::ColorType::Indexed);
        encoder.set_depth(png::BitDepth::Eight);

        // 设置调色板
        let plte: Vec<u8> = palette.iter().flat_map(|c| [c.r, c.g, c.b]).collect();
        let trns: Vec<u8> = palette.iter().map(|c| c.a).collect();
        encoder.set_palette(plte);
        encoder.set_trns(trns);

        let mut writer = encoder.write_header().context("PNG write_header failed")?;
        writer
            .write_image_data(&indexed_pixels)
            .context("PNG write_image_data failed")?;
    }

    // oxipng 优化
    let opts = oxipng_options(optimization_level);
    let optimized = oxipng::optimize_from_memory(&png_data, &opts)
        .context("oxipng optimization failed")?;

    Ok(optimized)
}

/// 用 image-rs 将 DynamicImage 编码为 PNG 字节
fn encode_to_png_bytes(image: &RawImage) -> Result<Vec<u8>> {
    let mut buf = Cursor::new(Vec::new());
    image
        .pixels
        .write_to(&mut buf, image::ImageFormat::Png)
        .context("Failed to encode as PNG")?;
    Ok(buf.into_inner())
}

/// 构建 oxipng 选项
fn oxipng_options(level: u8) -> oxipng::Options {
    let mut opts = oxipng::Options::from_preset(level.min(6));
    opts.strip = oxipng::StripChunks::Safe;
    opts
}
