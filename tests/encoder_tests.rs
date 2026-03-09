use image::{DynamicImage, RgbImage};
use std::path::PathBuf;
use tinyimg::engine::codec::universal::UniversalDecoder;
use tinyimg::engine::codec::{Decoder, EncodedOutput, Encoder};
use tinyimg::engine::params::{EncodeParams, ImageFormat};
use tinyimg::engine::raw_image::RawImage;

/// 创建一个测试用的 RawImage（100x100 渐变色）
fn create_test_image() -> RawImage {
    let mut img = RgbImage::new(100, 100);
    for y in 0..100 {
        for x in 0..100 {
            img.put_pixel(x, y, image::Rgb([x as u8 * 2, y as u8 * 2, 128]));
        }
    }
    RawImage::new(
        DynamicImage::ImageRgb8(img),
        ImageFormat::Jpeg,
        PathBuf::from("test.jpg"),
    )
}

#[test]
fn test_jpeg_encoder() {
    use tinyimg::engine::codec::jpeg::MozjpegEncoder;

    let image = create_test_image();
    let params = EncodeParams::Jpeg {
        quality: 80,
        progressive: true,
    };

    let result = MozjpegEncoder.encode(&image, &params).unwrap();
    assert_eq!(result.format, ImageFormat::Jpeg);
    assert!(!result.data.is_empty());
    // JPEG 应该以 FFD8FF 开头
    assert_eq!(&result.data[..3], &[0xFF, 0xD8, 0xFF]);
    // 压缩后应该比原始数据小
    assert!(result.data.len() < 100 * 100 * 3);
}

#[test]
fn test_png_encoder_lossless() {
    use tinyimg::engine::codec::png::OxipngEncoder;

    let image = create_test_image();
    let params = EncodeParams::Png {
        lossy: false,
        optimization_level: 2,
    };

    let result = OxipngEncoder.encode(&image, &params).unwrap();
    assert_eq!(result.format, ImageFormat::Png);
    assert!(!result.data.is_empty());
    // PNG 应该以 89504E47 开头
    assert_eq!(&result.data[..4], &[0x89, 0x50, 0x4E, 0x47]);
}

#[test]
fn test_png_encoder_lossy() {
    use tinyimg::engine::codec::png::OxipngEncoder;

    let image = create_test_image();
    let params = EncodeParams::Png {
        lossy: true,
        optimization_level: 2,
    };

    let result = OxipngEncoder.encode(&image, &params).unwrap();
    assert_eq!(result.format, ImageFormat::Png);
    assert!(!result.data.is_empty());
    assert_eq!(&result.data[..4], &[0x89, 0x50, 0x4E, 0x47]);
}

#[test]
fn test_webp_encoder() {
    use tinyimg::engine::codec::webp::WebpEncoder;

    let image = create_test_image();
    let params = EncodeParams::WebP {
        quality: 80,
        lossless: true,
    };

    let result = WebpEncoder.encode(&image, &params).unwrap();
    assert_eq!(result.format, ImageFormat::WebP);
    assert!(!result.data.is_empty());
    // WebP 应该以 RIFF...WEBP 开头
    assert_eq!(&result.data[..4], b"RIFF");
    assert_eq!(&result.data[8..12], b"WEBP");
}

#[test]
fn test_gif_encoder() {
    use tinyimg::engine::codec::gif::GifEncoder;

    let image = create_test_image();
    let params = EncodeParams::Gif {
        quality: 80,
        fast: false,
    };

    let result = GifEncoder.encode(&image, &params).unwrap();
    assert_eq!(result.format, ImageFormat::Gif);
    assert!(!result.data.is_empty());
    // GIF 应该以 GIF8 开头
    assert_eq!(&result.data[..4], b"GIF8");
}

#[test]
fn test_universal_decoder_jpeg() {
    use tinyimg::engine::codec::jpeg::MozjpegEncoder;

    // 先编码一张 JPEG
    let image = create_test_image();
    let params = EncodeParams::Jpeg {
        quality: 90,
        progressive: false,
    };
    let encoded = MozjpegEncoder.encode(&image, &params).unwrap();

    // 然后解码
    let decoded = UniversalDecoder
        .decode(&encoded.data, ImageFormat::Jpeg)
        .unwrap();
    assert_eq!(decoded.width(), 100);
    assert_eq!(decoded.height(), 100);
}

#[test]
fn test_avif_encoder() {
    use tinyimg::engine::codec::avif::AvifEncoder;

    let image = create_test_image();
    let params = EncodeParams::Avif {
        quality: 50,
        speed: 10, // 最快速度用于测试
    };

    let result = AvifEncoder.encode(&image, &params).unwrap();
    assert_eq!(result.format, ImageFormat::Avif);
    assert!(!result.data.is_empty());
}

#[test]
fn test_jxl_encoder_returns_error() {
    use tinyimg::engine::codec::jxl::JxlEncoder;

    let image = create_test_image();
    let params = EncodeParams::Jxl {
        quality: 80,
        effort: 7,
    };

    // JXL 编码应该返回错误（暂不支持）
    let result = JxlEncoder.encode(&image, &params);
    assert!(result.is_err());
}
