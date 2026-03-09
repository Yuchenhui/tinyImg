use crate::engine::codec::{Decoder, Encoder};
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
        let registry = Self {
            decoders: HashMap::new(),
            encoders: HashMap::new(),
        };
        // TODO: 注册所有内置编解码器
        // registry.register_decoder(ImageFormat::Jpeg, Box::new(JpegDecoder));
        // registry.register_encoder(ImageFormat::Jpeg, Box::new(MozjpegEncoder));
        // registry.register_encoder(ImageFormat::Png, Box::new(OxipngEncoder));
        // ...
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
