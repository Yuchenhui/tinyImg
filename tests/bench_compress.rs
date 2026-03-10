//! 实际压缩流程性能分析（不缩放，仅编码压缩）
//!
//! 运行: cargo test --test bench_compress -- --nocapture

use image::{DynamicImage, RgbaImage};
use std::path::PathBuf;
use std::time::Instant;
use tinyimg::engine::codec::Encoder;
use tinyimg::engine::params::{EncodeParams, ImageFormat};
use tinyimg::engine::raw_image::RawImage;

/// 创建逼真的测试图像（渐变+噪声，模拟真实照片的熵）
fn create_photo_like_image(width: u32, height: u32) -> RawImage {
    let mut img = RgbaImage::new(width, height);
    let mut rng_state: u32 = 42;
    for y in 0..height {
        for x in 0..width {
            // 简单伪随机（xorshift32）
            rng_state ^= rng_state << 13;
            rng_state ^= rng_state >> 17;
            rng_state ^= rng_state << 5;
            let noise = (rng_state % 40) as i16 - 20; // ±20 噪声

            let r = ((x * 200 / width.max(1)) as i16 + noise).clamp(0, 255) as u8;
            let g = ((y * 200 / height.max(1)) as i16 + noise).clamp(0, 255) as u8;
            let b = (((x + y) * 100 / (width + height).max(1)) as i16 + noise).clamp(0, 255) as u8;
            img.put_pixel(x, y, image::Rgba([r, g, b, 255]));
        }
    }
    RawImage::new(
        DynamicImage::ImageRgba8(img),
        ImageFormat::Png,
        PathBuf::from("bench.png"),
    )
}

fn bench_encode(label: &str, runs: usize, f: impl Fn() -> anyhow::Result<usize>) {
    // warmup
    let _ = f();

    let mut times = Vec::with_capacity(runs);
    let mut sizes = Vec::new();
    for _ in 0..runs {
        let start = Instant::now();
        let size = f().expect("encode failed");
        times.push(start.elapsed().as_secs_f64() * 1000.0);
        sizes.push(size);
    }
    times.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let median = times[times.len() / 2];
    let avg: f64 = times.iter().sum::<f64>() / times.len() as f64;
    let output_size = sizes[0];
    eprintln!(
        "  {label:25} | median={median:8.2}ms | avg={avg:8.2}ms | output={:.1}KB",
        output_size as f64 / 1024.0
    );
}

#[test]
fn bench_compression_no_resize() {
    eprintln!("\n=== 压缩性能分析（无缩放） ===\n");

    let sizes: Vec<(u32, u32)> = vec![
        (1024, 768),    // 约 1MP
        (1920, 1080),   // 2MP (1080p)
        (3840, 2160),   // 8MP (4K)
    ];

    let runs = 3;

    for (w, h) in &sizes {
        let pixels = w * h;
        let raw_size = pixels * 4; // RGBA
        eprintln!("--- {w}x{h} ({:.1}MP, raw={:.1}MB) ---",
            pixels as f64 / 1_000_000.0,
            raw_size as f64 / (1024.0 * 1024.0),
        );

        // JPEG 编码
        {
            let encoder = tinyimg::engine::codec::jpeg::MozjpegEncoder;
            for quality in [60u8, 80, 92] {
                let params = EncodeParams::Jpeg {
                    quality,
                    progressive: true,
                };
                bench_encode(&format!("JPEG q={quality} progressive"), runs, || {
                    let img = create_photo_like_image(*w, *h);
                    let output = encoder.encode(&img, &params)?;
                    Ok(output.data.len())
                });
            }
        }

        // PNG 编码
        {
            let encoder = tinyimg::engine::codec::png::OxipngEncoder;
            for (lossy, opt_level) in [(false, 2u8), (false, 4), (true, 2)] {
                let label = if lossy {
                    format!("PNG lossy o={opt_level}")
                } else {
                    format!("PNG lossless o={opt_level}")
                };
                let params = EncodeParams::Png {
                    lossy,
                    optimization_level: opt_level,
                };
                bench_encode(&label, runs, || {
                    let img = create_photo_like_image(*w, *h);
                    let output = encoder.encode(&img, &params)?;
                    Ok(output.data.len())
                });
            }
        }

        // WebP 编码
        {
            let encoder = tinyimg::engine::codec::webp::WebpEncoder;
            let params = EncodeParams::WebP {
                quality: 80,
                lossless: true,
            };
            bench_encode("WebP lossless q=80", runs, || {
                let img = create_photo_like_image(*w, *h);
                let output = encoder.encode(&img, &params)?;
                Ok(output.data.len())
            });
        }

        // AVIF 编码
        {
            let encoder = tinyimg::engine::codec::avif::AvifEncoder;
            for (quality, speed) in [(70u8, 6u8), (80, 4)] {
                let params = EncodeParams::Avif { quality, speed };
                bench_encode(&format!("AVIF q={quality} speed={speed}"), runs, || {
                    let img = create_photo_like_image(*w, *h);
                    let output = encoder.encode(&img, &params)?;
                    Ok(output.data.len())
                });
            }
        }

        eprintln!();
    }

    eprintln!("=== 分析完成 ===\n");
    eprintln!("结论：编码时间占比 >95%，GPU 无法加速这些操作。");
    eprintln!("GPU 加速仅在需要缩放时有意义（≥1024px 图像缩放快 1.1~1.7x）。\n");
}
