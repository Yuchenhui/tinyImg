use crate::engine::preprocess::Preprocessor;
use crate::engine::raw_image::RawImage;
use anyhow::{Context, Result};

/// CPU 图像缩放预处理器（基于 fast_image_resize）
pub struct CpuResizeProcessor {
    pub max_width: Option<u32>,
    pub max_height: Option<u32>,
}

impl CpuResizeProcessor {
    pub fn new(max_width: Option<u32>, max_height: Option<u32>) -> Self {
        Self {
            max_width,
            max_height,
        }
    }

    /// 计算目标尺寸（保持纵横比）
    fn compute_target_size(&self, src_w: u32, src_h: u32) -> Option<(u32, u32)> {
        let max_w = self.max_width.unwrap_or(src_w);
        let max_h = self.max_height.unwrap_or(src_h);

        if src_w <= max_w && src_h <= max_h {
            return None;
        }

        let scale_w = max_w as f64 / src_w as f64;
        let scale_h = max_h as f64 / src_h as f64;
        let scale = scale_w.min(scale_h);

        let dst_w = (src_w as f64 * scale).round() as u32;
        let dst_h = (src_h as f64 * scale).round() as u32;

        Some((dst_w.max(1), dst_h.max(1)))
    }
}

impl Preprocessor for CpuResizeProcessor {
    fn name(&self) -> &'static str {
        "cpu-resize"
    }

    fn process(&self, image: RawImage) -> Result<RawImage> {
        let src_w = image.width();
        let src_h = image.height();

        let (dst_w, dst_h) = match self.compute_target_size(src_w, src_h) {
            Some(size) => size,
            None => return Ok(image),
        };

        tracing::info!("CPU resize: {src_w}x{src_h} → {dst_w}x{dst_h}");

        // 使用 fast_image_resize 进行 SIMD 加速缩放
        let src_image = image.pixels.to_rgba8();

        let src_view = fast_image_resize::images::Image::from_vec_u8(
            src_w,
            src_h,
            src_image.into_raw(),
            fast_image_resize::PixelType::U8x4,
        )
        .context("Failed to create source image for resize")?;

        let mut dst_image = fast_image_resize::images::Image::new(
            dst_w,
            dst_h,
            fast_image_resize::PixelType::U8x4,
        );

        let mut resizer = fast_image_resize::Resizer::new();
        resizer
            .resize(
                &src_view,
                &mut dst_image,
                Some(&fast_image_resize::ResizeOptions::new().resize_alg(
                    fast_image_resize::ResizeAlg::Convolution(
                        fast_image_resize::FilterType::Lanczos3,
                    ),
                )),
            )
            .context("fast_image_resize failed")?;

        let resized_rgba =
            image::RgbaImage::from_raw(dst_w, dst_h, dst_image.into_vec())
                .context("Failed to create resized image")?;

        Ok(RawImage {
            pixels: image::DynamicImage::ImageRgba8(resized_rgba),
            source_format: image.source_format,
            metadata: image.metadata,
            source_path: image.source_path,
        })
    }
}
