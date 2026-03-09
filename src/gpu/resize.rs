use crate::engine::preprocess::Preprocessor;
use crate::engine::raw_image::RawImage;
use anyhow::{Context, Result};
use std::sync::Arc;
use wgpu::util::DeviceExt;

/// GPU 图像缩放预处理器（基于 wgpu compute shader）
///
/// 使用双线性插值在 GPU 上缩放图像。
/// Pipeline 和 BindGroupLayout 在首次使用时编译并缓存，避免重复开销。
pub struct GpuResizeProcessor {
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
    max_width: Option<u32>,
    max_height: Option<u32>,
    /// 缓存的 compute pipeline + bind group layout
    cached_pipeline: std::sync::OnceLock<CachedPipeline>,
}

struct CachedPipeline {
    pipeline: wgpu::ComputePipeline,
    bind_group_layout: wgpu::BindGroupLayout,
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct ResizeParams {
    src_width: u32,
    src_height: u32,
    dst_width: u32,
    dst_height: u32,
}

impl GpuResizeProcessor {
    pub fn new(
        device: Arc<wgpu::Device>,
        queue: Arc<wgpu::Queue>,
        max_width: Option<u32>,
        max_height: Option<u32>,
    ) -> Self {
        Self {
            device,
            queue,
            max_width,
            max_height,
            cached_pipeline: std::sync::OnceLock::new(),
        }
    }

    /// 获取或创建缓存的 pipeline
    fn get_pipeline(&self) -> &CachedPipeline {
        self.cached_pipeline.get_or_init(|| {
            let device = &self.device;

            let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("resize_bilinear"),
                source: wgpu::ShaderSource::Wgsl(RESIZE_SHADER.into()),
            });

            let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("resize_bind_group_layout"),
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
                label: Some("resize_pipeline_layout"),
                bind_group_layouts: &[&bind_group_layout],
                push_constant_ranges: &[],
            });

            let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("resize_pipeline"),
                layout: Some(&pipeline_layout),
                module: &shader,
                entry_point: Some("main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                cache: None,
            });

            CachedPipeline {
                pipeline,
                bind_group_layout,
            }
        })
    }

    /// 计算目标尺寸（保持纵横比）
    fn compute_target_size(&self, src_w: u32, src_h: u32) -> Option<(u32, u32)> {
        let max_w = self.max_width.unwrap_or(src_w);
        let max_h = self.max_height.unwrap_or(src_h);

        if src_w <= max_w && src_h <= max_h {
            return None; // 无需缩放
        }

        let scale_w = max_w as f64 / src_w as f64;
        let scale_h = max_h as f64 / src_h as f64;
        let scale = scale_w.min(scale_h);

        let dst_w = (src_w as f64 * scale).round() as u32;
        let dst_h = (src_h as f64 * scale).round() as u32;

        Some((dst_w.max(1), dst_h.max(1)))
    }

    /// 检查输入/输出 buffer 是否超出 GPU 限制
    fn exceeds_gpu_limit(&self, src_w: u32, src_h: u32, dst_w: u32, dst_h: u32) -> bool {
        let max_buffer = self.device.limits().max_storage_buffer_binding_size as u64;
        let input_bytes = (src_w as u64) * (src_h as u64) * 4;
        let output_bytes = (dst_w as u64) * (dst_h as u64) * 4;
        input_bytes > max_buffer || output_bytes > max_buffer
    }

    /// CPU 回退缩放（用于超大图超出 GPU buffer 限制的情况）
    fn cpu_resize_fallback(
        &self,
        image: &image::DynamicImage,
        dst_w: u32,
        dst_h: u32,
    ) -> image::DynamicImage {
        tracing::warn!(
            "Image too large for GPU resize, falling back to CPU (fast_image_resize)"
        );
        image.resize_exact(dst_w, dst_h, image::imageops::FilterType::Lanczos3)
    }

    /// 在 GPU 上执行缩放（使用缓存的 pipeline）
    fn gpu_resize(&self, rgba_data: &[u8], src_w: u32, src_h: u32, dst_w: u32, dst_h: u32) -> Result<Vec<u8>> {
        let device = &self.device;
        let queue = &self.queue;
        let cached = self.get_pipeline();

        // 创建 buffers（每次调用需要新 buffer，因为尺寸不同）
        let input_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("resize_input"),
            contents: rgba_data,
            usage: wgpu::BufferUsages::STORAGE,
        });

        let output_size = (dst_w * dst_h * 4) as u64;
        let output_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("resize_output"),
            size: output_size,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        let staging_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("resize_staging"),
            size: output_size,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let params = ResizeParams {
            src_width: src_w,
            src_height: src_h,
            dst_width: dst_w,
            dst_height: dst_h,
        };
        let params_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("resize_params"),
            contents: bytemuck::bytes_of(&params),
            usage: wgpu::BufferUsages::UNIFORM,
        });

        // 创建 bind group
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("resize_bind_group"),
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

        // 分发 compute shader
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("resize_encoder"),
        });

        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("resize_pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&cached.pipeline);
            pass.set_bind_group(0, &bind_group, &[]);

            // 8x8 工作组
            let workgroups_x = (dst_w + 7) / 8;
            let workgroups_y = (dst_h + 7) / 8;
            pass.dispatch_workgroups(workgroups_x, workgroups_y, 1);
        }

        // 拷贝输出到 staging buffer
        encoder.copy_buffer_to_buffer(&output_buffer, 0, &staging_buffer, 0, output_size);
        queue.submit(std::iter::once(encoder.finish()));

        // 读取结果
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
        let result = data.to_vec();
        drop(data);
        staging_buffer.unmap();

        Ok(result)
    }
}

impl Preprocessor for GpuResizeProcessor {
    fn name(&self) -> &'static str {
        "gpu-resize"
    }

    fn process(&self, image: RawImage) -> Result<RawImage> {
        let src_w = image.width();
        let src_h = image.height();

        let (dst_w, dst_h) = match self.compute_target_size(src_w, src_h) {
            Some(size) => size,
            None => return Ok(image), // 无需缩放
        };

        // 超大图回退 CPU
        if self.exceeds_gpu_limit(src_w, src_h, dst_w, dst_h) {
            let resized = self.cpu_resize_fallback(&image.pixels, dst_w, dst_h);
            return Ok(RawImage {
                pixels: resized,
                source_format: image.source_format,
                metadata: image.metadata,
                source_path: image.source_path,
            });
        }

        tracing::info!("GPU resize: {src_w}x{src_h} → {dst_w}x{dst_h}");

        let rgba = image.pixels.to_rgba8();
        let resized_data = self.gpu_resize(rgba.as_raw(), src_w, src_h, dst_w, dst_h)?;

        let resized_image = image::RgbaImage::from_raw(dst_w, dst_h, resized_data)
            .context("Failed to create resized image from GPU output")?;

        Ok(RawImage {
            pixels: image::DynamicImage::ImageRgba8(resized_image),
            source_format: image.source_format,
            metadata: image.metadata,
            source_path: image.source_path,
        })
    }
}

/// 使用 storage buffer 的双线性插值缩放 shader
/// （替代原来基于 texture 的版本，storage buffer 更通用）
const RESIZE_SHADER: &str = r#"
struct Params {
    src_width: u32,
    src_height: u32,
    dst_width: u32,
    dst_height: u32,
}

@group(0) @binding(0) var<storage, read> input: array<u32>;
@group(0) @binding(1) var<storage, read_write> output: array<u32>;
@group(0) @binding(2) var<uniform> params: Params;

// 从 u32 (RGBA packed) 解包为 vec4<f32>
fn unpack_rgba(packed: u32) -> vec4<f32> {
    return vec4<f32>(
        f32(packed & 0xFFu) / 255.0,
        f32((packed >> 8u) & 0xFFu) / 255.0,
        f32((packed >> 16u) & 0xFFu) / 255.0,
        f32((packed >> 24u) & 0xFFu) / 255.0,
    );
}

// 打包 vec4<f32> 为 u32
fn pack_rgba(color: vec4<f32>) -> u32 {
    let r = u32(clamp(color.r * 255.0, 0.0, 255.0));
    let g = u32(clamp(color.g * 255.0, 0.0, 255.0));
    let b = u32(clamp(color.b * 255.0, 0.0, 255.0));
    let a = u32(clamp(color.a * 255.0, 0.0, 255.0));
    return r | (g << 8u) | (b << 16u) | (a << 24u);
}

// 安全读取像素（边界 clamp）
fn sample_pixel(x: i32, y: i32) -> vec4<f32> {
    let cx = clamp(x, 0, i32(params.src_width) - 1);
    let cy = clamp(y, 0, i32(params.src_height) - 1);
    let idx = u32(cy) * params.src_width + u32(cx);
    return unpack_rgba(input[idx]);
}

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= params.dst_width || id.y >= params.dst_height) {
        return;
    }

    // 源图坐标（双线性插值）
    let src_x = (f32(id.x) + 0.5) * f32(params.src_width) / f32(params.dst_width) - 0.5;
    let src_y = (f32(id.y) + 0.5) * f32(params.src_height) / f32(params.dst_height) - 0.5;

    let x0 = i32(floor(src_x));
    let y0 = i32(floor(src_y));
    let fx = src_x - floor(src_x);
    let fy = src_y - floor(src_y);

    // 双线性插值 4 个相邻像素
    let p00 = sample_pixel(x0, y0);
    let p10 = sample_pixel(x0 + 1, y0);
    let p01 = sample_pixel(x0, y0 + 1);
    let p11 = sample_pixel(x0 + 1, y0 + 1);

    let color = mix(mix(p00, p10, fx), mix(p01, p11, fx), fy);

    let out_idx = id.y * params.dst_width + id.x;
    output[out_idx] = pack_rgba(color);
}
"#;
