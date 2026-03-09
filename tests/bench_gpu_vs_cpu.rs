//! GPU vs CPU 性能对比测试
//!
//! 运行: cargo test --features gpu --test bench_gpu_vs_cpu -- --nocapture

#![cfg(feature = "gpu")]

use image::{DynamicImage, RgbaImage};
use std::path::PathBuf;
use std::time::Instant;
use tinyimg::engine::params::ImageFormat;
use tinyimg::engine::preprocess::Preprocessor;
use tinyimg::engine::raw_image::RawImage;
use tinyimg::gpu::context::GpuAccelerator;

/// 创建指定尺寸的测试图像（渐变色 RGBA）
fn create_test_image(width: u32, height: u32) -> RawImage {
    let mut img = RgbaImage::new(width, height);
    for y in 0..height {
        for x in 0..width {
            img.put_pixel(
                x,
                y,
                image::Rgba([
                    (x * 255 / width.max(1)) as u8,
                    (y * 255 / height.max(1)) as u8,
                    ((x + y) * 128 / (width + height).max(1)) as u8,
                    255,
                ]),
            );
        }
    }
    RawImage::new(
        DynamicImage::ImageRgba8(img),
        ImageFormat::Png,
        PathBuf::from("bench.png"),
    )
}

/// 多次运行取中位数（排除编译/预热开销）
fn bench_resize<F: Fn() -> anyhow::Result<RawImage>>(
    label: &str,
    warmup: usize,
    runs: usize,
    f: F,
) -> Vec<f64> {
    // warmup
    for _ in 0..warmup {
        let _ = f();
    }

    let mut times = Vec::with_capacity(runs);
    for _ in 0..runs {
        let start = Instant::now();
        let result = f();
        let elapsed = start.elapsed().as_secs_f64() * 1000.0; // ms
        assert!(result.is_ok(), "Resize failed: {:?}", result.err());
        times.push(elapsed);
    }

    times.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let median = times[times.len() / 2];
    let min = times[0];
    let max = times[times.len() - 1];
    let avg: f64 = times.iter().sum::<f64>() / times.len() as f64;

    eprintln!(
        "  {label:20} | min={min:8.2}ms | median={median:8.2}ms | avg={avg:8.2}ms | max={max:8.2}ms"
    );

    times
}

#[test]
fn bench_gpu_vs_cpu_resize() {
    let gpu = GpuAccelerator::try_new_sync();
    if !gpu.is_available() {
        eprintln!("GPU not available, skipping benchmark");
        return;
    }
    eprintln!("\n=== GPU vs CPU Resize Benchmark ===");
    eprintln!("GPU: {}", gpu.name());

    let device = gpu.device.clone().unwrap();
    let queue = gpu.queue.clone().unwrap();

    // 测试不同尺寸
    let test_cases: Vec<(u32, u32, u32, u32)> = vec![
        (256, 256, 128, 128),       // 小图
        (1024, 1024, 512, 512),     // 中图
        (2048, 2048, 1024, 1024),   // 大图
        (4096, 4096, 2048, 2048),   // 超大图
        (3840, 2160, 1920, 1080),   // 4K → 1080p
        (1920, 1080, 800, 450),     // 1080p → 小图
    ];

    let runs = 5;
    let warmup = 2;

    for (src_w, src_h, max_w, max_h) in &test_cases {
        eprintln!(
            "\n--- {src_w}x{src_h} → max {max_w}x{max_h} ---"
        );

        let image = create_test_image(*src_w, *src_h);

        // CPU resize
        let cpu_processor = tinyimg::engine::preprocess::resize::CpuResizeProcessor::new(
            Some(*max_w),
            Some(*max_h),
        );
        let cpu_times = bench_resize("CPU (Lanczos3)", warmup, runs, || {
            let img = create_test_image(*src_w, *src_h);
            cpu_processor.process(img)
        });

        // GPU resize
        let gpu_processor = tinyimg::gpu::resize::GpuResizeProcessor::new(
            device.clone(),
            queue.clone(),
            Some(*max_w),
            Some(*max_h),
        );
        let gpu_times = bench_resize("GPU (Bilinear)", warmup, runs, || {
            let img = create_test_image(*src_w, *src_h);
            gpu_processor.process(img)
        });

        // 对比
        let cpu_median = cpu_times[cpu_times.len() / 2];
        let gpu_median = gpu_times[gpu_times.len() / 2];
        let speedup = cpu_median / gpu_median;

        if speedup >= 1.0 {
            eprintln!("  >>> GPU 快 {speedup:.2}x");
        } else {
            eprintln!("  >>> CPU 快 {:.2}x", 1.0 / speedup);
        }
    }

    eprintln!("\n=== Benchmark Complete ===\n");
}

#[test]
fn bench_gpu_vs_cpu_color_convert() {
    let gpu = GpuAccelerator::try_new_sync();
    if !gpu.is_available() {
        eprintln!("GPU not available, skipping benchmark");
        return;
    }
    eprintln!("\n=== GPU vs CPU Color Convert Benchmark ===");
    eprintln!("GPU: {}", gpu.name());

    let device = gpu.device.clone().unwrap();
    let queue = gpu.queue.clone().unwrap();

    let sizes: Vec<(u32, u32)> = vec![
        (512, 512),
        (1024, 1024),
        (2048, 2048),
        (4096, 4096),
    ];

    let runs = 5;
    let warmup = 2;

    for (w, h) in &sizes {
        eprintln!("\n--- {w}x{h} RGB→YCbCr ---");

        // GPU color convert
        let gpu_converter = tinyimg::gpu::color::GpuColorConverter::new(
            device.clone(),
            queue.clone(),
            tinyimg::gpu::color::ColorConvertDirection::RgbToYcbcr,
        );

        // 预检查是否超限
        let test_img = create_test_image(*w, *h);
        match gpu_converter.process(test_img) {
            Ok(_) => {
                bench_resize("GPU ColorConvert", warmup, runs, || {
                    let img = create_test_image(*w, *h);
                    gpu_converter.process(img)
                });
            }
            Err(e) => {
                eprintln!("  GPU ColorConvert     | SKIPPED: {e}");
            }
        }

        // CPU baseline: 简单的逐像素转换作为对比
        bench_resize("CPU ColorConvert", warmup, runs, || {
            let img = create_test_image(*w, *h);
            let rgba = img.pixels.to_rgba8();
            let mut out = rgba.clone();
            for pixel in out.pixels_mut() {
                let r = pixel[0] as f32;
                let g = pixel[1] as f32;
                let b = pixel[2] as f32;
                let y = 0.299 * r + 0.587 * g + 0.114 * b;
                let cb = 128.0 - 0.168736 * r - 0.331264 * g + 0.5 * b;
                let cr = 128.0 + 0.5 * r - 0.418688 * g - 0.081312 * b;
                pixel[0] = y.clamp(0.0, 255.0) as u8;
                pixel[1] = cb.clamp(0.0, 255.0) as u8;
                pixel[2] = cr.clamp(0.0, 255.0) as u8;
            }
            Ok(RawImage::new(
                DynamicImage::ImageRgba8(out),
                img.source_format,
                img.source_path,
            ))
        });
    }
}
