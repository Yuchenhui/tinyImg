use crate::engine::params::ImageFormat;
use image::DynamicImage;
use std::path::PathBuf;

/// 管线内统一的图像表示
///
/// 封装 image-rs 的 DynamicImage，附加元数据信息，
/// 作为 Decode → Preprocess → Encode 管线中流转的数据单元。
pub struct RawImage {
    /// 像素数据
    pub pixels: DynamicImage,
    /// 原始文件格式
    pub source_format: ImageFormat,
    /// 图像元数据（EXIF/ICC/XMP）
    pub metadata: ImageMetadata,
    /// 原始文件路径
    pub source_path: PathBuf,
}

impl RawImage {
    pub fn new(
        pixels: DynamicImage,
        source_format: ImageFormat,
        source_path: PathBuf,
    ) -> Self {
        Self {
            pixels,
            source_format,
            metadata: ImageMetadata::default(),
            source_path,
        }
    }

    pub fn width(&self) -> u32 {
        self.pixels.width()
    }

    pub fn height(&self) -> u32 {
        self.pixels.height()
    }
}

/// 图像元数据
#[derive(Debug, Clone, Default)]
pub struct ImageMetadata {
    /// 原始 EXIF 数据（字节）
    pub exif: Option<Vec<u8>>,
    /// ICC 色彩配置文件
    pub icc_profile: Option<Vec<u8>>,
    /// XMP 数据
    pub xmp: Option<Vec<u8>>,
}
