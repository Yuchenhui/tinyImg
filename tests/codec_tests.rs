use tinyimg::engine::params::ImageFormat;

#[test]
fn test_format_from_extension() {
    assert_eq!(ImageFormat::from_extension("jpg"), Some(ImageFormat::Jpeg));
    assert_eq!(ImageFormat::from_extension("jpeg"), Some(ImageFormat::Jpeg));
    assert_eq!(ImageFormat::from_extension("PNG"), Some(ImageFormat::Png));
    assert_eq!(ImageFormat::from_extension("webp"), Some(ImageFormat::WebP));
    assert_eq!(ImageFormat::from_extension("avif"), Some(ImageFormat::Avif));
    assert_eq!(ImageFormat::from_extension("jxl"), Some(ImageFormat::Jxl));
    assert_eq!(ImageFormat::from_extension("gif"), Some(ImageFormat::Gif));
    assert_eq!(ImageFormat::from_extension("svg"), Some(ImageFormat::Svg));
    assert_eq!(ImageFormat::from_extension("bmp"), None);
}

#[test]
fn test_format_from_magic_bytes() {
    // JPEG
    assert_eq!(
        ImageFormat::from_magic_bytes(&[0xFF, 0xD8, 0xFF, 0xE0]),
        Some(ImageFormat::Jpeg)
    );
    // PNG
    assert_eq!(
        ImageFormat::from_magic_bytes(&[0x89, 0x50, 0x4E, 0x47]),
        Some(ImageFormat::Png)
    );
    // GIF
    assert_eq!(
        ImageFormat::from_magic_bytes(b"GIF89a"),
        Some(ImageFormat::Gif)
    );
    // Unknown
    assert_eq!(
        ImageFormat::from_magic_bytes(&[0x00, 0x01, 0x02, 0x03]),
        None
    );
}

#[test]
fn test_format_extension_roundtrip() {
    let formats = [
        ImageFormat::Jpeg,
        ImageFormat::Png,
        ImageFormat::WebP,
        ImageFormat::Avif,
        ImageFormat::Jxl,
        ImageFormat::Gif,
        ImageFormat::Svg,
    ];
    for fmt in formats {
        let ext = fmt.extension();
        assert_eq!(ImageFormat::from_extension(ext), Some(fmt));
    }
}
