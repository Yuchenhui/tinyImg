//! GPU 加速测试
//!
//! 这些测试需要 GPU 可用。在无 GPU 环境下会自动跳过。
//! 运行: cargo test --features gpu --test gpu_tests

#![cfg(feature = "gpu")]

use image::{DynamicImage, RgbaImage};
use std::path::PathBuf;
use tinyimg::engine::params::ImageFormat;
use tinyimg::engine::preprocess::Preprocessor;
use tinyimg::engine::raw_image::RawImage;
use tinyimg::gpu::context::GpuAccelerator;

/// 安全获取 GPU 加速器，如果不可用则跳过测试
fn require_gpu() -> Option<GpuAccelerator> {
    let gpu = GpuAccelerator::try_new_sync();
    if !gpu.is_available() {
        eprintln!("GPU not available, skipping test");
        None
    } else {
        eprintln!("GPU: {}", gpu.name());
        Some(gpu)
    }
}

/// 创建测试用 RGBA 图像（指定尺寸，渐变色）
fn create_test_rgba(width: u32, height: u32) -> RawImage {
    let mut img = RgbaImage::new(width, height);
    for y in 0..height {
        for x in 0..width {
            img.put_pixel(
                x,
                y,
                image::Rgba([
                    (x * 255 / width.max(1)) as u8,
                    (y * 255 / height.max(1)) as u8,
                    128,
                    255,
                ]),
            );
        }
    }
    RawImage::new(
        DynamicImage::ImageRgba8(img),
        ImageFormat::Png,
        PathBuf::from("test.png"),
    )
}

#[test]
fn test_gpu_resize() {
    let Some(gpu) = require_gpu() else { return };

    let device = gpu.device.clone().unwrap();
    let queue = gpu.queue.clone().unwrap();

    let processor = tinyimg::gpu::resize::GpuResizeProcessor::new(
        device,
        queue,
        Some(50),  // max 50px width
        Some(50),  // max 50px height
    );

    let image = create_test_rgba(200, 100);
    assert_eq!(image.width(), 200);
    assert_eq!(image.height(), 100);

    let resized = processor.process(image).expect("GPU resize failed");
    assert_eq!(resized.width(), 50);
    assert_eq!(resized.height(), 25); // 保持纵横比
}

#[test]
fn test_gpu_resize_no_op() {
    let Some(gpu) = require_gpu() else { return };

    let device = gpu.device.clone().unwrap();
    let queue = gpu.queue.clone().unwrap();

    let processor = tinyimg::gpu::resize::GpuResizeProcessor::new(
        device,
        queue,
        Some(500),
        Some(500),
    );

    let image = create_test_rgba(100, 100);
    let result = processor.process(image).expect("GPU resize failed");
    // 不需要缩放，尺寸应保持不变
    assert_eq!(result.width(), 100);
    assert_eq!(result.height(), 100);
}

#[test]
fn test_gpu_color_convert() {
    let Some(gpu) = require_gpu() else { return };

    let device = gpu.device.clone().unwrap();
    let queue = gpu.queue.clone().unwrap();

    // RGB → YCbCr → RGB 往返应近似还原
    let to_ycbcr = tinyimg::gpu::color::GpuColorConverter::new(
        device.clone(),
        queue.clone(),
        tinyimg::gpu::color::ColorConvertDirection::RgbToYcbcr,
    );
    let to_rgb = tinyimg::gpu::color::GpuColorConverter::new(
        device,
        queue,
        tinyimg::gpu::color::ColorConvertDirection::YcbcrToRgb,
    );

    let image = create_test_rgba(64, 64);
    let original_pixels = image.pixels.to_rgba8().clone();

    let ycbcr = to_ycbcr.process(image).expect("RGB→YCbCr failed");
    let roundtrip = to_rgb.process(ycbcr).expect("YCbCr→RGB failed");
    let result_pixels = roundtrip.pixels.to_rgba8();

    // 检查往返后像素值误差在可接受范围内（量化误差）
    let mut max_diff = 0u8;
    for (orig, result) in original_pixels.pixels().zip(result_pixels.pixels()) {
        for c in 0..3 {
            let diff = (orig[c] as i16 - result[c] as i16).unsigned_abs() as u8;
            max_diff = max_diff.max(diff);
        }
    }
    // 允许 ±3 的量化误差（f32 精度 + u8 量化截断）
    assert!(
        max_diff <= 3,
        "RGB→YCbCr→RGB roundtrip error too large: max_diff={max_diff}"
    );
}

#[test]
fn test_gpu_dct_basic() {
    let Some(gpu) = require_gpu() else { return };

    let device = gpu.device.clone().unwrap();
    let queue = gpu.queue.clone().unwrap();

    let dct = tinyimg::gpu::dct::GpuDct::new(device, queue);

    // 创建一个简单的 8x8 全灰色块
    let width = 8u32;
    let height = 8u32;
    let data: Vec<f32> = vec![128.0; (width * height) as usize];

    let result = dct.forward_dct(&data, width, height).expect("DCT failed");

    // 全灰色（128）减去 128 后全为 0，DC 系数应为 0，所有 AC 系数也为 0
    assert_eq!(result.len(), 64);
    for &coeff in &result {
        assert!(
            coeff.abs() < 1.0,
            "Expected near-zero DCT coefficients for uniform block, got {coeff}"
        );
    }
}

#[test]
fn test_create_resize_processor_fallback() {
    // 不启用 GPU 时应 fallback 到 CPU
    let gpu = GpuAccelerator::unavailable();
    let processor = tinyimg::gpu::create_resize_processor(&gpu, Some(100), Some(100));
    assert_eq!(processor.name(), "cpu-resize");
}
