pub mod context;

#[cfg(feature = "gpu")]
pub mod resize;

#[cfg(feature = "gpu")]
pub mod color;

#[cfg(feature = "gpu")]
pub mod dct;

#[cfg(feature = "gpu")]
pub mod jpeg;

use crate::engine::codec::Encoder;
use crate::engine::preprocess::Preprocessor;
use context::GpuAccelerator;

/// 根据 GPU 可用性创建缩放预处理器
///
/// GPU 可用时返回 GPU 版本，否则返回 CPU 版本（fast_image_resize）。
pub fn create_resize_processor(
    gpu: &GpuAccelerator,
    max_width: Option<u32>,
    max_height: Option<u32>,
) -> Box<dyn Preprocessor> {
    #[cfg(feature = "gpu")]
    {
        if gpu.is_available() {
            if let (Some(device), Some(queue)) = (gpu.device.clone(), gpu.queue.clone()) {
                tracing::info!("Using GPU resize processor");
                return Box::new(resize::GpuResizeProcessor::new(
                    device, queue, max_width, max_height,
                ));
            }
        }
    }

    let _ = gpu; // suppress unused warning when gpu feature is off
    tracing::info!("Using CPU resize processor");
    Box::new(crate::engine::preprocess::resize::CpuResizeProcessor::new(
        max_width, max_height,
    ))
}

/// 根据 GPU 可用性创建 JPEG 编码器
///
/// GPU 可用时返回 GPU JPEG 编码器（速度快 2-7x，体积略大），
/// 否则返回 mozjpeg CPU 编码器（体积最优）。
pub fn create_jpeg_encoder(gpu: &GpuAccelerator) -> Box<dyn Encoder> {
    #[cfg(feature = "gpu")]
    {
        if gpu.is_available() {
            if let (Some(device), Some(queue)) = (gpu.device.clone(), gpu.queue.clone()) {
                tracing::info!("Using GPU JPEG encoder");
                return Box::new(jpeg::GpuJpegEncoder::new(device, queue));
            }
        }
    }

    let _ = gpu;
    tracing::info!("Using CPU JPEG encoder (mozjpeg)");
    Box::new(crate::engine::codec::jpeg::MozjpegEncoder)
}
