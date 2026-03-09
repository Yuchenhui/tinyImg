use serde::{Deserialize, Serialize};

/// 压缩预设
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressionPreset {
    pub name: String,
    pub description: String,
    pub jpeg: JpegPreset,
    pub png: PngPreset,
    pub webp: WebpPreset,
    pub avif: AvifPreset,
    pub jxl: JxlPreset,
    pub gif: GifPreset,
    pub resize: Option<ResizePreset>,
    pub strip_metadata: bool,
    /// 是否为内置预设（不可删除）
    #[serde(default)]
    pub builtin: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JpegPreset {
    pub quality: u8,
    pub progressive: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PngPreset {
    pub lossy: bool,
    pub optimization_level: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebpPreset {
    pub quality: u8,
    pub lossless: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AvifPreset {
    pub quality: u8,
    pub speed: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JxlPreset {
    pub quality: u8,
    pub effort: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GifPreset {
    pub quality: u8,
    pub fast: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResizePreset {
    pub max_width: Option<u32>,
    pub max_height: Option<u32>,
    pub maintain_aspect_ratio: bool,
}

impl CompressionPreset {
    /// 内置预设：Web 优化
    pub fn web_optimized() -> Self {
        Self {
            name: "web_optimized".into(),
            description: "Web 优化 - 平衡压缩率和质量".into(),
            jpeg: JpegPreset { quality: 80, progressive: true },
            png: PngPreset { lossy: true, optimization_level: 2 },
            webp: WebpPreset { quality: 80, lossless: false },
            avif: AvifPreset { quality: 70, speed: 6 },
            jxl: JxlPreset { quality: 75, effort: 7 },
            gif: GifPreset { quality: 80, fast: false },
            resize: None,
            strip_metadata: true,
            builtin: true,
        }
    }

    /// 内置预设：高质量
    pub fn high_quality() -> Self {
        Self {
            name: "high_quality".into(),
            description: "高质量 - 最小视觉损失".into(),
            jpeg: JpegPreset { quality: 92, progressive: true },
            png: PngPreset { lossy: false, optimization_level: 4 },
            webp: WebpPreset { quality: 90, lossless: false },
            avif: AvifPreset { quality: 85, speed: 4 },
            jxl: JxlPreset { quality: 90, effort: 7 },
            gif: GifPreset { quality: 90, fast: false },
            resize: None,
            strip_metadata: false,
            builtin: true,
        }
    }

    /// 内置预设：最小体积
    pub fn smallest_size() -> Self {
        Self {
            name: "smallest_size".into(),
            description: "最小体积 - 激进压缩".into(),
            jpeg: JpegPreset { quality: 60, progressive: true },
            png: PngPreset { lossy: true, optimization_level: 6 },
            webp: WebpPreset { quality: 50, lossless: false },
            avif: AvifPreset { quality: 50, speed: 8 },
            jxl: JxlPreset { quality: 60, effort: 9 },
            gif: GifPreset { quality: 60, fast: true },
            resize: None,
            strip_metadata: true,
            builtin: true,
        }
    }

    /// 内置预设：无损
    pub fn lossless() -> Self {
        Self {
            name: "lossless".into(),
            description: "无损 - 不损失任何画质".into(),
            jpeg: JpegPreset { quality: 100, progressive: true },
            png: PngPreset { lossy: false, optimization_level: 4 },
            webp: WebpPreset { quality: 100, lossless: true },
            avif: AvifPreset { quality: 100, speed: 4 },
            jxl: JxlPreset { quality: 100, effort: 7 },
            gif: GifPreset { quality: 100, fast: false },
            resize: None,
            strip_metadata: false,
            builtin: true,
        }
    }

    /// 返回所有内置预设
    pub fn builtin_presets() -> Vec<Self> {
        vec![
            Self::web_optimized(),
            Self::high_quality(),
            Self::smallest_size(),
            Self::lossless(),
        ]
    }
}
