use crate::engine::codec::{Codec, EncodedOutput, Encoder};
use crate::engine::params::{EncodeParams, ImageFormat};
use crate::engine::raw_image::RawImage;
use anyhow::{bail, Result};

/// JPEG XL 编解码器
///
/// jxl-oxide 目前仅支持解码。JPEG XL 编码暂不可用。
/// 未来可通过 jxl-enc 或其他 crate 添加编码支持。
///
/// 当前用途：支持 JXL 输入文件解码（通过 UniversalDecoder 或专用解码器），
/// 然后转换输出为其他格式。
pub struct JxlEncoder;

impl Codec for JxlEncoder {
    fn name(&self) -> &'static str {
        "jxl-oxide (decode-only)"
    }

    fn formats(&self) -> &[ImageFormat] {
        &[ImageFormat::Jxl]
    }
}

impl Encoder for JxlEncoder {
    fn encode(&self, _image: &RawImage, params: &EncodeParams) -> Result<EncodedOutput> {
        let EncodeParams::Jxl { .. } = params else {
            bail!("JxlEncoder requires Jxl params");
        };

        bail!("JPEG XL encoding is not yet supported. jxl-oxide is a decoder-only library. \
               Please choose a different output format (JPEG, PNG, WebP, or AVIF).")
    }
}
