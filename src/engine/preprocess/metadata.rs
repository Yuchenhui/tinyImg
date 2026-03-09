use crate::engine::preprocess::Preprocessor;
use crate::engine::raw_image::RawImage;
use anyhow::Result;

/// 元数据剥离预处理器（EXIF/ICC/XMP）
pub struct MetadataStripper {
    pub strip_exif: bool,
    pub strip_icc: bool,
    pub strip_xmp: bool,
}

impl MetadataStripper {
    pub fn strip_all() -> Self {
        Self {
            strip_exif: true,
            strip_icc: true,
            strip_xmp: true,
        }
    }
}

impl Preprocessor for MetadataStripper {
    fn name(&self) -> &'static str {
        "metadata-strip"
    }

    fn process(&self, mut image: RawImage) -> Result<RawImage> {
        if self.strip_exif {
            image.metadata.exif = None;
        }
        if self.strip_icc {
            image.metadata.icc_profile = None;
        }
        if self.strip_xmp {
            image.metadata.xmp = None;
        }
        Ok(image)
    }
}
