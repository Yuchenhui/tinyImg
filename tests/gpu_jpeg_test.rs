//! GPU JPEG 编码器正确性和性能测试
//!
//! 运行: cargo test --features gpu --test gpu_jpeg_test -- --nocapture

#![cfg(feature = "gpu")]

use image::{DynamicImage, RgbaImage};
use std::path::PathBuf;
use std::time::Instant;
use tinyimg::engine::codec::Encoder;
use tinyimg::engine::params::{EncodeParams, ImageFormat};
use tinyimg::engine::raw_image::RawImage;
use tinyimg::gpu::context::GpuAccelerator;

fn create_test_image(width: u32, height: u32) -> RawImage {
    let mut img = RgbaImage::new(width, height);
    let mut rng: u32 = 12345;
    for y in 0..height {
        for x in 0..width {
            rng ^= rng << 13;
            rng ^= rng >> 17;
            rng ^= rng << 5;
            let noise = (rng % 30) as i16 - 15;
            let r = ((x * 200 / width.max(1)) as i16 + noise).clamp(0, 255) as u8;
            let g = ((y * 200 / height.max(1)) as i16 + noise).clamp(0, 255) as u8;
            let b = (((x + y) * 100 / (width + height).max(1)) as i16 + noise).clamp(0, 255) as u8;
            img.put_pixel(x, y, image::Rgba([r, g, b, 255]));
        }
    }
    RawImage::new(DynamicImage::ImageRgba8(img), ImageFormat::Jpeg, PathBuf::from("test.jpg"))
}

#[test]
fn test_gpu_jpeg_encode_basic() {
    let gpu = GpuAccelerator::try_new_sync();
    if !gpu.is_available() {
        eprintln!("GPU not available, skipping");
        return;
    }
    eprintln!("GPU: {}", gpu.name());

    let device = gpu.device.clone().unwrap();
    let queue = gpu.queue.clone().unwrap();
    let encoder = tinyimg::gpu::jpeg::GpuJpegEncoder::new(device, queue);

    let image = create_test_image(64, 64);
    let params = EncodeParams::Jpeg { quality: 80, progressive: false };

    let result = encoder.encode(&image, &params).expect("GPU JPEG encode failed");
    eprintln!("Encoded 64x64 → {} bytes", result.data.len());

    // 验证 JPEG magic bytes
    assert_eq!(&result.data[0..2], &[0xFF, 0xD8], "Missing SOI marker");
    assert_eq!(&result.data[result.data.len()-2..], &[0xFF, 0xD9], "Missing EOI marker");

    // 验证能被 image-rs 解码
    let decoded = image::load_from_memory_with_format(&result.data, image::ImageFormat::Jpeg);
    assert!(decoded.is_ok(), "GPU JPEG output is not valid JPEG: {:?}", decoded.err());

    let dec = decoded.unwrap();
    assert_eq!(dec.width(), 64);
    assert_eq!(dec.height(), 64);
    eprintln!("Decode verified: {}x{}", dec.width(), dec.height());
}

#[test]
fn test_gpu_jpeg_encode_various_sizes() {
    let gpu = GpuAccelerator::try_new_sync();
    if !gpu.is_available() {
        eprintln!("GPU not available, skipping");
        return;
    }

    let device = gpu.device.clone().unwrap();
    let queue = gpu.queue.clone().unwrap();
    let encoder = tinyimg::gpu::jpeg::GpuJpegEncoder::new(device, queue);
    let params = EncodeParams::Jpeg { quality: 75, progressive: false };

    // 测试不同尺寸（包括非 8 整除的）
    let sizes = [(8, 8), (16, 16), (100, 75), (640, 480), (1920, 1080)];

    for (w, h) in sizes {
        let image = create_test_image(w, h);
        let start = Instant::now();
        let result = encoder.encode(&image, &params);
        let elapsed = start.elapsed().as_secs_f64() * 1000.0;

        match result {
            Ok(output) => {
                // 验证能解码
                let decoded = image::load_from_memory_with_format(&output.data, image::ImageFormat::Jpeg);
                assert!(decoded.is_ok(), "GPU JPEG {w}x{h} not decodable: {:?}", decoded.err());
                let dec = decoded.unwrap();
                // 4:2:0 子采样可能导致尺寸被 MCU 对齐，所以宽高可能略有差异
                // 但标准 JPEG 解码器应该返回 header 中声明的尺寸
                assert_eq!(dec.width(), w, "Width mismatch for {w}x{h}");
                assert_eq!(dec.height(), h, "Height mismatch for {w}x{h}");
                eprintln!(
                    "  {w}x{h}: {:.1}ms → {:.1}KB ✓",
                    elapsed,
                    output.data.len() as f64 / 1024.0
                );
            }
            Err(e) => {
                panic!("GPU JPEG encode failed for {w}x{h}: {e}");
            }
        }
    }
}

#[test]
fn bench_gpu_vs_mozjpeg() {
    let gpu = GpuAccelerator::try_new_sync();
    if !gpu.is_available() {
        eprintln!("GPU not available, skipping");
        return;
    }
    eprintln!("\n=== GPU JPEG vs mozjpeg 性能对比 ===");
    eprintln!("GPU: {}", gpu.name());

    let device = gpu.device.clone().unwrap();
    let queue = gpu.queue.clone().unwrap();
    let gpu_encoder = tinyimg::gpu::jpeg::GpuJpegEncoder::new(device, queue);
    let cpu_encoder = tinyimg::engine::codec::jpeg::MozjpegEncoder;
    let params = EncodeParams::Jpeg { quality: 80, progressive: true };

    let sizes = [(640, 480), (1920, 1080), (3840, 2160)];
    let runs = 3;

    for (w, h) in sizes {
        eprintln!("\n--- {w}x{h} ---");

        // GPU warmup + bench
        let image = create_test_image(w, h);
        let _ = gpu_encoder.encode(&image, &params); // warmup

        let mut gpu_times = Vec::new();
        let mut gpu_size = 0;
        for _ in 0..runs {
            let img = create_test_image(w, h);
            let start = Instant::now();
            let result = gpu_encoder.encode(&img, &params).unwrap();
            gpu_times.push(start.elapsed().as_secs_f64() * 1000.0);
            gpu_size = result.data.len();
        }
        gpu_times.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let gpu_median = gpu_times[gpu_times.len() / 2];

        // CPU bench
        let _ = cpu_encoder.encode(&image, &params); // warmup
        let mut cpu_times = Vec::new();
        let mut cpu_size = 0;
        for _ in 0..runs {
            let img = create_test_image(w, h);
            let start = Instant::now();
            let result = cpu_encoder.encode(&img, &params).unwrap();
            cpu_times.push(start.elapsed().as_secs_f64() * 1000.0);
            cpu_size = result.data.len();
        }
        cpu_times.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let cpu_median = cpu_times[cpu_times.len() / 2];

        let speedup = cpu_median / gpu_median;
        eprintln!(
            "  GPU: {:.1}ms ({:.1}KB) | CPU(mozjpeg): {:.1}ms ({:.1}KB)",
            gpu_median,
            gpu_size as f64 / 1024.0,
            cpu_median,
            cpu_size as f64 / 1024.0,
        );
        if speedup >= 1.0 {
            eprintln!("  >>> GPU 快 {speedup:.2}x (体积差 {:.0}%)", (gpu_size as f64 / cpu_size as f64 - 1.0) * 100.0);
        } else {
            eprintln!("  >>> CPU 快 {:.2}x", 1.0 / speedup);
        }
    }
}
