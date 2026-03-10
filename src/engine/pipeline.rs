use crate::engine::codec::{Decoder, Encoder};
use crate::engine::params::{EncodeParams, ImageFormat};
use crate::engine::preprocess::Preprocessor;
use anyhow::{Context, Result};
use std::path::Path;

/// 三阶段压缩管线：Decode → Preprocess Chain → Encode
///
/// Pipeline 是无状态的，可安全传入 rayon 线程池并行执行。
pub struct CompressionPipeline {
    decoder: Box<dyn Decoder>,
    preprocessors: Vec<Box<dyn Preprocessor>>,
    encoder: Box<dyn Encoder>,
    params: EncodeParams,
}

impl CompressionPipeline {
    pub fn new(
        decoder: Box<dyn Decoder>,
        preprocessors: Vec<Box<dyn Preprocessor>>,
        encoder: Box<dyn Encoder>,
        params: EncodeParams,
    ) -> Self {
        Self {
            decoder,
            preprocessors,
            encoder,
            params,
        }
    }

    /// 执行压缩管线
    pub fn run(&self, input_path: &Path) -> Result<CompressionResult> {
        let data = std::fs::read(input_path)
            .with_context(|| format!("Failed to read: {}", input_path.display()))?;

        let source_format = ImageFormat::from_magic_bytes(&data)
            .or_else(|| {
                input_path
                    .extension()
                    .and_then(|e| e.to_str())
                    .and_then(ImageFormat::from_extension)
            })
            .context("Unable to detect image format")?;

        // Stage 1: Decode
        let mut image = self
            .decoder
            .decode(&data, source_format)
            .context("Decode failed")?;

        // Stage 2: Preprocess chain
        for processor in &self.preprocessors {
            image = processor
                .process(image)
                .with_context(|| format!("Preprocessor '{}' failed", processor.name()))?;
        }

        // Stage 3: Encode
        let output = self
            .encoder
            .encode(&image, &self.params)
            .context("Encode failed")?;

        Ok(CompressionResult {
            compressed_size: output.data.len() as u64,
            data: output.data,
        })
    }
}

/// 压缩结果
pub struct CompressionResult {
    pub compressed_size: u64,
    pub data: Vec<u8>,
}
