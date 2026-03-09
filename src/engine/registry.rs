use crate::engine::codec::avif::AvifEncoder;
use crate::engine::codec::gif::GifEncoder;
use crate::engine::codec::jpeg::MozjpegEncoder;
use crate::engine::codec::jxl::JxlEncoder;
use crate::engine::codec::png::OxipngEncoder;
use crate::engine::codec::svg::SvgOptimizer;
use crate::engine::codec::universal::UniversalDecoder;
use crate::engine::codec::webp::WebpEncoder;
use crate::engine::codec::{Codec, Decoder, Encoder};
use crate::engine::params::ImageFormat;
use std::collections::HashMap;

/// 编解码器注册表
///
/// 管理所有格式的编码器和解码器实例。Pipeline 通过 Registry 获取对应格式的处理器。
pub struct CodecRegistry {
    decoders: HashMap<ImageFormat, Box<dyn Decoder>>,
    encoders: HashMap<ImageFormat, Box<dyn Encoder>>,
}

impl CodecRegistry {
    pub fn new() -> Self {
        let mut registry = Self {
            decoders: HashMap::new(),
            encoders: HashMap::new(),
        };

        // 注册通用解码器（image-rs 支持 JPEG/PNG/WebP/GIF）
        let universal = UniversalDecoder;
        for &fmt in universal.formats() {
            // 每种格式注册同一个解码器类型的独立实例
            registry.register_decoder(fmt, Box::new(UniversalDecoder));
        }
        // AVIF 也用 image-rs 解码（image 0.25 支持 AVIF 解码）
        registry.register_decoder(ImageFormat::Avif, Box::new(UniversalDecoder));

        // 注册编码器
        registry.register_encoder(ImageFormat::Jpeg, Box::new(MozjpegEncoder));
        registry.register_encoder(ImageFormat::Png, Box::new(OxipngEncoder));
        registry.register_encoder(ImageFormat::WebP, Box::new(WebpEncoder));
        registry.register_encoder(ImageFormat::Avif, Box::new(AvifEncoder));
        registry.register_encoder(ImageFormat::Gif, Box::new(GifEncoder));
        registry.register_encoder(ImageFormat::Jxl, Box::new(JxlEncoder));
        registry.register_encoder(ImageFormat::Svg, Box::new(SvgOptimizer));

        registry
    }

    pub fn register_decoder(&mut self, format: ImageFormat, decoder: Box<dyn Decoder>) {
        self.decoders.insert(format, decoder);
    }

    pub fn register_encoder(&mut self, format: ImageFormat, encoder: Box<dyn Encoder>) {
        self.encoders.insert(format, encoder);
    }

    pub fn get_decoder(&self, format: &ImageFormat) -> Option<&dyn Decoder> {
        self.decoders.get(format).map(|d| d.as_ref())
    }

    pub fn get_encoder(&self, format: &ImageFormat) -> Option<&dyn Encoder> {
        self.encoders.get(format).map(|e| e.as_ref())
    }

    pub fn supported_decode_formats(&self) -> Vec<ImageFormat> {
        self.decoders.keys().copied().collect()
    }

    pub fn supported_encode_formats(&self) -> Vec<ImageFormat> {
        self.encoders.keys().copied().collect()
    }
}

impl Default for CodecRegistry {
    fn default() -> Self {
        Self::new()
    }
}
