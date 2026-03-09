use crate::engine::preprocess::Preprocessor;
use crate::engine::raw_image::RawImage;
use anyhow::Result;

/// GPU 图像缩放预处理器（基于 wgpu compute shader）
pub struct GpuResizeProcessor {
    // TODO: 持有 wgpu device/queue 引用和预编译的 pipeline
}

impl Preprocessor for GpuResizeProcessor {
    fn name(&self) -> &'static str {
        "gpu-resize"
    }

    fn process(&self, image: RawImage) -> Result<RawImage> {
        // TODO: 实现 wgpu compute shader 缩放
        todo!("Implement GPU resize with wgpu compute shader")
    }
}
