use crate::engine::codec::{Codec, Encoder, EncodedOutput};
use crate::engine::params::{EncodeParams, ImageFormat};
use crate::engine::raw_image::RawImage;
use anyhow::{bail, Result};

/// oxipng 无损 PNG 优化器
pub struct OxipngEncoder;

impl Codec for OxipngEncoder {
    fn name(&self) -> &'static str {
        "oxipng"
    }

    fn formats(&self) -> &[ImageFormat] {
        &[ImageFormat::Png]
    }
}

impl Encoder for OxipngEncoder {
    fn encode(&self, image: &RawImage, params: &EncodeParams) -> Result<EncodedOutput> {
        let EncodeParams::Png {
            lossy,
            optimization_level,
        } = params
        else {
            bail!("OxipngEncoder requires Png params");
        };

        // TODO: 如果 lossy=true，先走 imagequant 量化
        // TODO: 然后用 oxipng 优化
        let _ = (image, lossy, optimization_level);
        todo!("Implement oxipng encoding")
    }
}
