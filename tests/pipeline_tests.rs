use tinyimg::engine::params::{ImageFormat, EncodeParams};
use tinyimg::engine::pipeline::CompressionResult;

#[test]
fn test_compression_result_ratio() {
    let result = CompressionResult {
        original_size: 1000,
        compressed_size: 700,
        data: vec![0; 700],
        output_format: ImageFormat::Jpeg,
    };
    assert!((result.compression_ratio() - 0.3).abs() < 0.001);
    assert_eq!(result.bytes_saved(), 300);
}

#[test]
fn test_compression_result_zero_original() {
    let result = CompressionResult {
        original_size: 0,
        compressed_size: 0,
        data: vec![],
        output_format: ImageFormat::Png,
    };
    assert_eq!(result.compression_ratio(), 0.0);
}
