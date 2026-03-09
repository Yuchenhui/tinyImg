use crate::engine::preprocess::Preprocessor;
use crate::engine::raw_image::RawImage;
use anyhow::{Context, Result};
use std::sync::Arc;
use wgpu::util::DeviceExt;

/// GPU 色彩空间转换预处理器
///
/// 使用 compute shader 在 GPU 上进行 RGB↔YCbCr 转换。
/// Pipeline 在首次使用时编译并缓存。
pub struct GpuColorConverter {
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
    direction: ColorConvertDirection,
    cached_pipeline: std::sync::OnceLock<CachedColorPipeline>,
}

struct CachedColorPipeline {
    pipeline: wgpu::ComputePipeline,
    bind_group_layout: wgpu::BindGroupLayout,
}

#[derive(Clone, Copy)]
pub enum ColorConvertDirection {
    RgbToYcbcr = 0,
    YcbcrToRgb = 1,
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct ColorParams {
    width: u32,
    height: u32,
    direction: u32,
    _padding: u32,
}

impl GpuColorConverter {
    pub fn new(
        device: Arc<wgpu::Device>,
        queue: Arc<wgpu::Queue>,
        direction: ColorConvertDirection,
    ) -> Self {
        Self {
            device,
            queue,
            direction,
            cached_pipeline: std::sync::OnceLock::new(),
        }
    }

    /// 获取或创建缓存的 pipeline
    fn get_pipeline(&self) -> &CachedColorPipeline {
        self.cached_pipeline.get_or_init(|| {
            let device = &self.device;

            let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("color_convert"),
                source: wgpu::ShaderSource::Wgsl(COLOR_SHADER.into()),
            });

            let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("color_bind_group_layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: false },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
            });

            let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("color_pipeline_layout"),
                bind_group_layouts: &[&bind_group_layout],
                push_constant_ranges: &[],
            });

            let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("color_pipeline"),
                layout: Some(&pipeline_layout),
                module: &shader,
                entry_point: Some("main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                cache: None,
            });

            CachedColorPipeline {
                pipeline,
                bind_group_layout,
            }
        })
    }

    fn gpu_convert(&self, rgba_data: &[u8], width: u32, height: u32) -> Result<Vec<u8>> {
        let device = &self.device;
        let queue = &self.queue;
        let cached = self.get_pipeline();

        // 将 u8 RGBA 数据转为 f32 以便 shader 处理
        let float_data: Vec<f32> = rgba_data
            .iter()
            .map(|&b| b as f32 / 255.0)
            .collect();

        let input_bytes = bytemuck::cast_slice(&float_data);

        // 检查是否超过 GPU buffer 大小限制（默认 128MB）
        let max_buffer_size = device.limits().max_storage_buffer_binding_size as u64;
        if input_bytes.len() as u64 > max_buffer_size {
            anyhow::bail!(
                "Image too large for GPU color conversion: {}MB > {}MB limit. Use CPU fallback.",
                input_bytes.len() / (1024 * 1024),
                max_buffer_size / (1024 * 1024)
            );
        }
        let input_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("color_input"),
            contents: input_bytes,
            usage: wgpu::BufferUsages::STORAGE,
        });

        let output_size = input_bytes.len() as u64;
        let output_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("color_output"),
            size: output_size,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        let staging_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("color_staging"),
            size: output_size,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let params = ColorParams {
            width,
            height,
            direction: self.direction as u32,
            _padding: 0,
        };
        let params_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("color_params"),
            contents: bytemuck::bytes_of(&params),
            usage: wgpu::BufferUsages::UNIFORM,
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("color_bind_group"),
            layout: &cached.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: input_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: output_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: params_buffer.as_entire_binding(),
                },
            ],
        });

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("color_encoder"),
        });

        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("color_pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&cached.pipeline);
            pass.set_bind_group(0, &bind_group, &[]);

            // 使用 2D dispatch 避免 65535 限制
            let workgroups_x = (width + 15) / 16;
            let workgroups_y = (height + 15) / 16;
            pass.dispatch_workgroups(workgroups_x, workgroups_y, 1);
        }

        encoder.copy_buffer_to_buffer(&output_buffer, 0, &staging_buffer, 0, output_size);
        queue.submit(std::iter::once(encoder.finish()));

        let buffer_slice = staging_buffer.slice(..);
        let (sender, receiver) = std::sync::mpsc::channel();
        buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
            sender.send(result).ok();
        });
        device.poll(wgpu::Maintain::Wait);

        receiver
            .recv()
            .context("GPU buffer map channel closed")?
            .context("GPU buffer map failed")?;

        let data = buffer_slice.get_mapped_range();
        let float_result: &[f32] = bytemuck::cast_slice(&data);

        // f32 → u8
        let result: Vec<u8> = float_result
            .iter()
            .map(|&f| (f * 255.0).clamp(0.0, 255.0) as u8)
            .collect();

        drop(data);
        staging_buffer.unmap();

        Ok(result)
    }
}

impl Preprocessor for GpuColorConverter {
    fn name(&self) -> &'static str {
        "gpu-color-convert"
    }

    fn process(&self, image: RawImage) -> Result<RawImage> {
        let rgba = image.pixels.to_rgba8();
        let (width, height) = rgba.dimensions();

        tracing::info!("GPU color conversion: {width}x{height}");

        let converted = self.gpu_convert(rgba.as_raw(), width, height)?;

        let converted_image = image::RgbaImage::from_raw(width, height, converted)
            .context("Failed to create image from GPU color conversion output")?;

        Ok(RawImage {
            pixels: image::DynamicImage::ImageRgba8(converted_image),
            source_format: image.source_format,
            metadata: image.metadata,
            source_path: image.source_path,
        })
    }
}

/// RGB↔YCbCr 色彩空间转换 compute shader（2D dispatch）
const COLOR_SHADER: &str = r#"
struct Params {
    width: u32,
    height: u32,
    direction: u32,
    _padding: u32,
}

@group(0) @binding(0) var<storage, read> input: array<vec4<f32>>;
@group(0) @binding(1) var<storage, read_write> output: array<vec4<f32>>;
@group(0) @binding(2) var<uniform> params: Params;

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= params.width || id.y >= params.height) {
        return;
    }

    let idx = id.y * params.width + id.x;
    let pixel = input[idx];

    if (params.direction == 0u) {
        // RGB → YCbCr (BT.601)
        let y  =  0.299 * pixel.r + 0.587 * pixel.g + 0.114 * pixel.b;
        let cb = -0.169 * pixel.r - 0.331 * pixel.g + 0.500 * pixel.b + 0.5;
        let cr =  0.500 * pixel.r - 0.419 * pixel.g - 0.081 * pixel.b + 0.5;
        output[idx] = vec4<f32>(y, cb, cr, pixel.a);
    } else {
        // YCbCr → RGB (BT.601)
        let y  = pixel.r;
        let cb = pixel.g - 0.5;
        let cr = pixel.b - 0.5;
        let r = y + 1.402 * cr;
        let g = y - 0.344 * cb - 0.714 * cr;
        let b = y + 1.772 * cb;
        output[idx] = vec4<f32>(
            clamp(r, 0.0, 1.0),
            clamp(g, 0.0, 1.0),
            clamp(b, 0.0, 1.0),
            pixel.a
        );
    }
}
"#;
