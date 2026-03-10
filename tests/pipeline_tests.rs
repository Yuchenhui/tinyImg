use tinyimg::engine::pipeline::CompressionResult;

#[test]
fn test_compression_result_ratio() {
    let result = CompressionResult {
        compressed_size: 700,
        data: vec![0; 700],
    };
    // CompressionResult 只有 compressed_size 和 data
    assert_eq!(result.compressed_size, 700);
    assert_eq!(result.data.len(), 700);
}

#[test]
fn test_compression_result_zero() {
    let result = CompressionResult {
        compressed_size: 0,
        data: vec![],
    };
    assert_eq!(result.compressed_size, 0);
    assert!(result.data.is_empty());
}
