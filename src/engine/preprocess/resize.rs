use crate::engine::preprocess::Preprocessor;
use crate::engine::raw_image::RawImage;
use anyhow::Result;

/// CPU 图像缩放预处理器（基于 fast_image_resize）
pub struct CpuResizeProcessor {
    pub max_width: Option<u32>,
    pub max_height: Option<u32>,
    pub maintain_aspect_ratio: bool,
}

impl CpuResizeProcessor {
    pub fn new(max_width: Option<u32>, max_height: Option<u32>) -> Self {
        Self {
            max_width,
            max_height,
            maintain_aspect_ratio: true,
        }
    }

    /// 判断是否需要缩放
    fn needs_resize(&self, width: u32, height: u32) -> bool {
        if let Some(max_w) = self.max_width {
            if width > max_w {
                return true;
            }
        }
        if let Some(max_h) = self.max_height {
            if height > max_h {
                return true;
            }
        }
        false
    }
}

impl Preprocessor for CpuResizeProcessor {
    fn name(&self) -> &'static str {
        "cpu-resize"
    }

    fn process(&self, image: RawImage) -> Result<RawImage> {
        if !self.needs_resize(image.width(), image.height()) {
            return Ok(image);
        }

        // TODO: 使用 fast_image_resize 实现缩放
        todo!("Implement CPU resize with fast_image_resize")
    }
}
