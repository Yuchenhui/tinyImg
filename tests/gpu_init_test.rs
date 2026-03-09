//! GPU 初始化和基础 compute shader 测试
#![cfg(feature = "gpu")]

use tinyimg::gpu::context::GpuAccelerator;
use tinyimg::engine::preprocess::Preprocessor;

#[test]
fn test_gpu_accelerator_init() {
    let gpu = GpuAccelerator::try_new_sync();
    eprintln!("GPU available: {}", gpu.is_available());
    if gpu.is_available() {
        eprintln!("GPU name: {}", gpu.name());
    }
}

#[test]
fn test_gpu_resize_minimal() {
    let gpu = GpuAccelerator::try_new_sync();
    if !gpu.is_available() {
        eprintln!("GPU not available, skipping");
        return;
    }

    let device = gpu.device.clone().unwrap();
    let queue = gpu.queue.clone().unwrap();

    eprintln!("Creating GpuResizeProcessor...");
    let processor = tinyimg::gpu::resize::GpuResizeProcessor::new(
        device, queue, Some(4), Some(4),
    );

    // 极小的 8x8 RGBA 图像
    eprintln!("Creating 8x8 test image...");
    let mut img = image::RgbaImage::new(8, 8);
    for p in img.pixels_mut() {
        *p = image::Rgba([128, 64, 32, 255]);
    }
    let raw = tinyimg::engine::raw_image::RawImage::new(
        image::DynamicImage::ImageRgba8(img),
        tinyimg::engine::params::ImageFormat::Png,
        std::path::PathBuf::from("test.png"),
    );

    eprintln!("Running GPU resize...");
    match processor.process(raw) {
        Ok(result) => {
            eprintln!("GPU resize ok: {}x{}", result.width(), result.height());
            assert_eq!(result.width(), 4);
            assert_eq!(result.height(), 4);
        }
        Err(e) => {
            eprintln!("GPU resize failed: {e}");
            panic!("GPU resize failed: {e}");
        }
    }
}

#[test]
fn test_wgpu_instance_creation() {
    eprintln!("Creating wgpu instance...");
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
        backends: wgpu::Backends::VULKAN | wgpu::Backends::DX12,
        ..Default::default()
    });
    eprintln!("Instance created, requesting adapter...");

    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        ..Default::default()
    }));

    match adapter {
        Some(adapter) => {
            let info = adapter.get_info();
            eprintln!("Adapter: {} ({:?})", info.name, info.backend);

            eprintln!("Requesting device...");
            match pollster::block_on(adapter.request_device(
                &wgpu::DeviceDescriptor::default(),
                None,
            )) {
                Ok((device, _queue)) => {
                    eprintln!("Device created successfully!");
                    // 简单测试：创建一个小 buffer
                    let _buf = device.create_buffer(&wgpu::BufferDescriptor {
                        label: Some("test"),
                        size: 256,
                        usage: wgpu::BufferUsages::STORAGE,
                        mapped_at_creation: false,
                    });
                    eprintln!("Buffer created successfully!");
                }
                Err(e) => {
                    eprintln!("Device creation failed: {e}");
                }
            }
        }
        None => {
            eprintln!("No GPU adapter found");
        }
    }
}
