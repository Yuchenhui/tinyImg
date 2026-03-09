use crate::engine::codec::{Codec, Encoder, EncodedOutput};
use crate::engine::params::{EncodeParams, ImageFormat};
use crate::engine::raw_image::RawImage;
use anyhow::{bail, Result};

/// gifski AGPL GIF 编码器
pub struct GifskiEncoder;

impl Codec for GifskiEncoder {
    fn name(&self) -> &'static str {
        "gifski"
    }

    fn formats(&self) -> &[ImageFormat] {
        &[ImageFormat::Gif]
    }
}

impl Encoder for GifskiEncoder {
    fn encode(&self, image: &RawImage, params: &EncodeParams) -> Result<EncodedOutput> {
        let EncodeParams::Gif { quality, fast } = params else {
            bail!("GifskiEncoder requires Gif params");
        };

        // TODO: 实现 gifski 编码
        let _ = (image, quality, fast);
        todo!("Implement GIF encoding")
    }
}
