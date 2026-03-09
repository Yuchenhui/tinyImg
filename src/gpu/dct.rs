use anyhow::{Context, Result};
use std::sync::Arc;
use wgpu::util::DeviceExt;

/// GPU DCT 正变换加速（用于 JPEG 编码前的 8x8 块变换）
///
/// 实现 8x8 块 DCT-II 正变换。每个工作组处理一个 8x8 块。
/// 对于高分辨率图像，GPU 并行 DCT 比 CPU 串行快数倍。
pub struct GpuDct {
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct DctParams {
    width: u32,
    height: u32,
    blocks_x: u32,
    blocks_y: u32,
}

impl GpuDct {
    pub fn new(device: Arc<wgpu::Device>, queue: Arc<wgpu::Queue>) -> Self {
        Self { device, queue }
    }

    /// 对单通道灰度数据执行 8x8 块 DCT 正变换
    ///
    /// 输入：width x height 的 f32 像素值 (0.0~255.0)
    /// 输出：DCT 系数（与输入同尺寸，按 8x8 块排列）
    pub fn forward_dct(&self, data: &[f32], width: u32, height: u32) -> Result<Vec<f32>> {
        let device = &self.device;
        let queue = &self.queue;

        let blocks_x = (width + 7) / 8;
        let blocks_y = (height + 7) / 8;

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("dct_forward"),
            source: wgpu::ShaderSource::Wgsl(DCT_SHADER.into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("dct_bind_group_layout"),
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
            label: Some("dct_pipeline_layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("dct_pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: Some("main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });

        // 确保数据对齐到 blocks 大小
        let padded_w = blocks_x * 8;
        let padded_h = blocks_y * 8;
        let mut padded_data = vec![0.0f32; (padded_w * padded_h) as usize];
        for y in 0..height {
            for x in 0..width {
                padded_data[(y * padded_w + x) as usize] = data[(y * width + x) as usize];
            }
        }

        let input_bytes = bytemuck::cast_slice(&padded_data);
        let input_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("dct_input"),
            contents: input_bytes,
            usage: wgpu::BufferUsages::STORAGE,
        });

        let output_size = input_bytes.len() as u64;
        let output_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("dct_output"),
            size: output_size,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        let staging_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("dct_staging"),
            size: output_size,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let params = DctParams {
            width: padded_w,
            height: padded_h,
            blocks_x,
            blocks_y,
        };
        let params_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("dct_params"),
            contents: bytemuck::bytes_of(&params),
            usage: wgpu::BufferUsages::UNIFORM,
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("dct_bind_group"),
            layout: &bind_group_layout,
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
            label: Some("dct_encoder"),
        });

        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("dct_pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            pass.dispatch_workgroups(blocks_x, blocks_y, 1);
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

        let mapped = buffer_slice.get_mapped_range();
        let result: Vec<f32> = bytemuck::cast_slice(&mapped).to_vec();
        drop(mapped);
        staging_buffer.unmap();

        // 裁剪回原始大小
        let mut output = vec![0.0f32; (width * height) as usize];
        for y in 0..height {
            for x in 0..width {
                output[(y * width + x) as usize] = result[(y * padded_w + x) as usize];
            }
        }

        Ok(output)
    }
}

/// 8x8 块 DCT-II 正变换 compute shader
///
/// 每个工作组（8x8 线程）处理图像中的一个 8x8 块。
/// 使用分离式 DCT：先行变换，再列变换。
const DCT_SHADER: &str = r#"
struct Params {
    width: u32,
    height: u32,
    blocks_x: u32,
    blocks_y: u32,
}

@group(0) @binding(0) var<storage, read> input: array<f32>;
@group(0) @binding(1) var<storage, read_write> output: array<f32>;
@group(0) @binding(2) var<uniform> params: Params;

const PI: f32 = 3.14159265358979323846;

// DCT-II 系数归一化因子
fn alpha(k: u32) -> f32 {
    if (k == 0u) {
        return sqrt(1.0 / 8.0);
    } else {
        return sqrt(2.0 / 8.0);
    }
}

var<workgroup> temp: array<f32, 64>;  // 8x8 共享内存

@compute @workgroup_size(8, 8)
fn main(
    @builtin(global_invocation_id) gid: vec3<u32>,
    @builtin(local_invocation_id) lid: vec3<u32>,
    @builtin(workgroup_id) wid: vec3<u32>,
) {
    // 当前块在图像中的基地址
    let block_x = wid.x * 8u;
    let block_y = wid.y * 8u;
    let local_idx = lid.y * 8u + lid.x;

    // 读取输入像素到共享内存（减去 128 居中）
    let px = block_x + lid.x;
    let py = block_y + lid.y;
    let src_idx = py * params.width + px;
    temp[local_idx] = input[src_idx] - 128.0;

    workgroupBarrier();

    // 行 DCT：每个线程计算一个 (row=lid.y, freq=lid.x) 位置的系数
    var row_sum: f32 = 0.0;
    let u = lid.x;  // 频率索引
    let row = lid.y;
    for (var n = 0u; n < 8u; n = n + 1u) {
        let val = temp[row * 8u + n];
        row_sum = row_sum + val * cos(PI * (f32(n) + 0.5) * f32(u) / 8.0);
    }
    row_sum = row_sum * alpha(u);

    workgroupBarrier();
    temp[local_idx] = row_sum;
    workgroupBarrier();

    // 列 DCT：每个线程计算一个 (freq_v=lid.y, freq_u=lid.x) 位置的系数
    var col_sum: f32 = 0.0;
    let v = lid.y;  // 垂直频率索引
    let col = lid.x;
    for (var m = 0u; m < 8u; m = m + 1u) {
        let val = temp[m * 8u + col];
        col_sum = col_sum + val * cos(PI * (f32(m) + 0.5) * f32(v) / 8.0);
    }
    col_sum = col_sum * alpha(v);

    // 写回输出
    let dst_idx = (block_y + lid.y) * params.width + (block_x + lid.x);
    output[dst_idx] = col_sum;
}
"#;
