use crate::engine::codec::{Codec, EncodedOutput, Encoder};
use crate::engine::params::{EncodeParams, ImageFormat};
use crate::engine::raw_image::RawImage;
use anyhow::{bail, Result};

/// SVG 优化器（需要 svgo sidecar）
///
/// SVG 优化通过外部 svgo 进程实现。
/// 当前版本暂未集成 svgo sidecar，后续添加。
pub struct SvgOptimizer;

impl Codec for SvgOptimizer {
    fn name(&self) -> &'static str {
        "svgo"
    }

    fn formats(&self) -> &[ImageFormat] {
        &[ImageFormat::Svg]
    }
}

impl Encoder for SvgOptimizer {
    fn encode(&self, _image: &RawImage, params: &EncodeParams) -> Result<EncodedOutput> {
        let EncodeParams::Svg { .. } = params else {
            bail!("SvgOptimizer requires Svg params");
        };

        bail!("SVG optimization is not yet implemented. \
               svgo sidecar integration will be added in a future version.")
    }
}
