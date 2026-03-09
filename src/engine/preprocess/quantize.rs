use crate::engine::preprocess::Preprocessor;
use crate::engine::raw_image::RawImage;
use anyhow::Result;

/// imagequant 调色板量化预处理器（用于 PNG 有损压缩）
///
/// 注意：imagequant 使用 GPL v3 许可证
pub struct PaletteQuantizer {
    pub quality: u8,
}

impl PaletteQuantizer {
    pub fn new(quality: u8) -> Self {
        Self { quality }
    }
}

impl Preprocessor for PaletteQuantizer {
    fn name(&self) -> &'static str {
        "imagequant"
    }

    fn process(&self, image: RawImage) -> Result<RawImage> {
        // TODO: 使用 imagequant v4 进行调色板量化
        let _ = self.quality;
        todo!("Implement imagequant palette quantization")
    }
}
