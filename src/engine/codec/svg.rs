use crate::engine::codec::{Codec, Encoder, EncodedOutput};
use crate::engine::params::{EncodeParams, ImageFormat};
use crate::engine::raw_image::RawImage;
use anyhow::{bail, Result};

/// svgo SVG 优化器（通过 sidecar 或 WASM 调用）
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
    fn encode(&self, image: &RawImage, params: &EncodeParams) -> Result<EncodedOutput> {
        let EncodeParams::Svg {
            multipass,
            precision,
        } = params
        else {
            bail!("SvgOptimizer requires Svg params");
        };

        // TODO: 通过 sidecar 进程调用 svgo
        let _ = (image, multipass, precision);
        todo!("Implement SVG optimization via svgo sidecar")
    }
}
