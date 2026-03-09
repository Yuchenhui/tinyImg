use crate::engine::codec::{Codec, Encoder, EncodedOutput};
use crate::engine::params::{EncodeParams, ImageFormat};
use crate::engine::raw_image::RawImage;
use anyhow::{bail, Result};

/// jxl-oxide 纯 Rust JPEG XL 编码器
pub struct JxlEncoder;

impl Codec for JxlEncoder {
    fn name(&self) -> &'static str {
        "jxl-oxide"
    }

    fn formats(&self) -> &[ImageFormat] {
        &[ImageFormat::Jxl]
    }
}

impl Encoder for JxlEncoder {
    fn encode(&self, image: &RawImage, params: &EncodeParams) -> Result<EncodedOutput> {
        let EncodeParams::Jxl { quality, effort } = params else {
            bail!("JxlEncoder requires Jxl params");
        };

        // TODO: 实现 jxl-oxide 编码
        let _ = (image, quality, effort);
        todo!("Implement JPEG XL encoding")
    }
}
