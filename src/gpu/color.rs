use crate::engine::preprocess::Preprocessor;
use crate::engine::raw_image::RawImage;
use anyhow::Result;

/// GPU 色彩空间转换预处理器
pub struct GpuColorConverter {
    // TODO: 持有 wgpu pipeline
}

impl Preprocessor for GpuColorConverter {
    fn name(&self) -> &'static str {
        "gpu-color-convert"
    }

    fn process(&self, image: RawImage) -> Result<RawImage> {
        // TODO: 实现 wgpu compute shader 色彩转换
        todo!("Implement GPU color conversion")
    }
}
