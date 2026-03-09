use crate::engine::preprocess::Preprocessor;
use crate::engine::raw_image::RawImage;
use anyhow::Result;

/// 色彩空间转换预处理器
pub struct ColorConverter;

impl Preprocessor for ColorConverter {
    fn name(&self) -> &'static str {
        "color-convert"
    }

    fn process(&self, image: RawImage) -> Result<RawImage> {
        // TODO: 实现色彩空间转换（RGB↔YCbCr/Lab）
        Ok(image)
    }
}
