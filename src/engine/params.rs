use serde::{Deserialize, Serialize};
use std::fmt;

/// 支持的图像格式
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ImageFormat {
    Jpeg,
    Png,
    WebP,
    Avif,
    Jxl,
    Gif,
    Svg,
}

impl ImageFormat {
    /// 从文件扩展名推断格式
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext.to_ascii_lowercase().as_str() {
            "jpg" | "jpeg" => Some(Self::Jpeg),
            "png" | "apng" => Some(Self::Png),
            "webp" => Some(Self::WebP),
            "avif" => Some(Self::Avif),
            "jxl" => Some(Self::Jxl),
            "gif" => Some(Self::Gif),
            "svg" | "svgz" => Some(Self::Svg),
            _ => None,
        }
    }

    /// 获取格式的默认文件扩展名
    pub fn extension(&self) -> &'static str {
        match self {
            Self::Jpeg => "jpg",
            Self::Png => "png",
            Self::WebP => "webp",
            Self::Avif => "avif",
            Self::Jxl => "jxl",
            Self::Gif => "gif",
            Self::Svg => "svg",
        }
    }

    /// 从文件头魔数探测格式
    pub fn from_magic_bytes(data: &[u8]) -> Option<Self> {
        if data.len() < 4 {
            return None;
        }
        match data {
            d if d.starts_with(&[0xFF, 0xD8, 0xFF]) => Some(Self::Jpeg),
            d if d.starts_with(&[0x89, 0x50, 0x4E, 0x47]) => Some(Self::Png),
            d if d.len() >= 12 && &d[0..4] == b"RIFF" && &d[8..12] == b"WEBP" => {
                Some(Self::WebP)
            }
            d if d.len() >= 12 && &d[4..12] == b"ftypavif" => Some(Self::Avif),
            d if d.starts_with(&[0xFF, 0x0A]) || d.starts_with(&[0x00, 0x00, 0x00, 0x0C]) => {
                Some(Self::Jxl)
            }
            d if d.starts_with(b"GIF8") => Some(Self::Gif),
            d if d.starts_with(b"<?xml") || d.starts_with(b"<svg") => Some(Self::Svg),
            _ => None,
        }
    }
}

impl fmt::Display for ImageFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Jpeg => write!(f, "JPEG"),
            Self::Png => write!(f, "PNG"),
            Self::WebP => write!(f, "WebP"),
            Self::Avif => write!(f, "AVIF"),
            Self::Jxl => write!(f, "JPEG XL"),
            Self::Gif => write!(f, "GIF"),
            Self::Svg => write!(f, "SVG"),
        }
    }
}

/// 编码参数（每种格式独立的参数集）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EncodeParams {
    Jpeg {
        quality: u8,
        progressive: bool,
    },
    Png {
        lossy: bool,
        optimization_level: u8,
    },
    WebP {
        quality: u8,
        lossless: bool,
    },
    Avif {
        quality: u8,
        speed: u8,
    },
    Jxl {
        quality: u8,
        effort: u8,
    },
    Gif {
        quality: u8,
        fast: bool,
    },
    Svg {
        multipass: bool,
        precision: u8,
    },
    /// 保持原格式原参数
    Passthrough,
}

impl EncodeParams {
    /// 获取参数对应的输出格式
    pub fn output_format(&self) -> Option<ImageFormat> {
        match self {
            Self::Jpeg { .. } => Some(ImageFormat::Jpeg),
            Self::Png { .. } => Some(ImageFormat::Png),
            Self::WebP { .. } => Some(ImageFormat::WebP),
            Self::Avif { .. } => Some(ImageFormat::Avif),
            Self::Jxl { .. } => Some(ImageFormat::Jxl),
            Self::Gif { .. } => Some(ImageFormat::Gif),
            Self::Svg { .. } => Some(ImageFormat::Svg),
            Self::Passthrough => None,
        }
    }
}

/// 输出格式选择
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OutputFormat {
    /// 保持原格式
    Original,
    /// 转换为指定格式
    Convert(ImageFormat),
}

/// 缩放参数
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResizeParams {
    pub max_width: Option<u32>,
    pub max_height: Option<u32>,
    pub maintain_aspect_ratio: bool,
}

/// 完整的压缩选项（用户侧接口）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressionOptions {
    pub output_format: OutputFormat,
    pub encode_params: EncodeParams,
    pub resize: Option<ResizeParams>,
    pub strip_metadata: bool,
}
