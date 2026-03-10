//! GPU 加速 JPEG 编码管线
//!
//! 流程: RGB→YCbCr(CPU) → 色度下采样(CPU) → DCT+量化+Zigzag(GPU) → Huffman(CPU) → JFIF 组装(CPU)
//!
//! GPU 负责计算密集的 DCT + 量化（每个 8x8 块独立并行），
//! CPU 负责本质串行的霍夫曼编码和码流组装。

use crate::engine::codec::{EncodedOutput, Encoder};
use crate::engine::params::EncodeParams;
use crate::engine::raw_image::RawImage;
use anyhow::{bail, Context, Result};
use std::sync::Arc;
use wgpu::util::DeviceExt;

/// GPU 加速 JPEG 编码器
pub struct GpuJpegEncoder {
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
    cached_pipeline: std::sync::OnceLock<CachedJpegPipeline>,
}

struct CachedJpegPipeline {
    pipeline: wgpu::ComputePipeline,
    bind_group_layout: wgpu::BindGroupLayout,
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct JpegDctParams {
    blocks_x: u32,
    blocks_y: u32,
    total_blocks: u32,
    _pad: u32,
}

impl GpuJpegEncoder {
    pub fn new(device: Arc<wgpu::Device>, queue: Arc<wgpu::Queue>) -> Self {
        Self {
            device,
            queue,
            cached_pipeline: std::sync::OnceLock::new(),
        }
    }

    fn get_pipeline(&self) -> &CachedJpegPipeline {
        self.cached_pipeline.get_or_init(|| {
            let device = &self.device;

            let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("jpeg_dct_quantize"),
                source: wgpu::ShaderSource::Wgsl(JPEG_DCT_QUANTIZE_SHADER.into()),
            });

            let bind_group_layout =
                device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("jpeg_bind_group_layout"),
                    entries: &[
                        // binding 0: input pixel data (f32, level-shifted)
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
                        // binding 1: output quantized zigzag coefficients (i32)
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
                        // binding 2: quantization table (f32 x 64)
                        wgpu::BindGroupLayoutEntry {
                            binding: 2,
                            visibility: wgpu::ShaderStages::COMPUTE,
                            ty: wgpu::BindingType::Buffer {
                                ty: wgpu::BufferBindingType::Storage { read_only: true },
                                has_dynamic_offset: false,
                                min_binding_size: None,
                            },
                            count: None,
                        },
                        // binding 3: params uniform
                        wgpu::BindGroupLayoutEntry {
                            binding: 3,
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
                label: Some("jpeg_pipeline_layout"),
                bind_group_layouts: &[&bind_group_layout],
                push_constant_ranges: &[],
            });

            let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("jpeg_pipeline"),
                layout: Some(&pipeline_layout),
                module: &shader,
                entry_point: Some("main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                cache: None,
            });

            CachedJpegPipeline {
                pipeline,
                bind_group_layout,
            }
        })
    }

    /// 对单通道数据执行 GPU DCT + 量化 + Zigzag
    ///
    /// 输入: padded_w x padded_h 的 f32 像素 (0~255)
    /// 量化表: 64 个 f32 值（已按 quality 缩放）
    /// 输出: blocks_x * blocks_y * 64 个 i32 量化系数（zigzag 顺序）
    ///
    /// 当数据超过 GPU `max_storage_buffer_binding_size`（通常 128MB）时，
    /// 自动按行分块处理，避免单次 dispatch 超限。
    fn gpu_dct_quantize(
        &self,
        channel_data: &[f32],
        blocks_x: u32,
        blocks_y: u32,
        quant_table: &[f32; 64],
    ) -> Result<Vec<i32>> {
        let total_blocks = (blocks_x as u64) * (blocks_y as u64);
        let input_bytes = total_blocks * 64 * 4; // f32
        let output_bytes = total_blocks * 64 * 4; // i32

        // 查询 GPU 限制（保守使用 90% 避免边界问题）
        let max_buffer = self.device.limits().max_storage_buffer_binding_size as u64;
        let safe_limit = max_buffer * 9 / 10;

        // 需要分块的条件：输入或输出 buffer 超出安全限制
        let needs_chunking = input_bytes > safe_limit || output_bytes > safe_limit;

        if !needs_chunking {
            return self.gpu_dct_quantize_single(channel_data, blocks_x, blocks_y, quant_table);
        }

        // 按行分块：计算每个 chunk 的最大行数
        let bytes_per_row = (blocks_x as u64) * 64 * 4;
        let max_rows_per_chunk = (safe_limit / bytes_per_row).max(1) as u32;

        tracing::debug!(
            "Large image: {}x{} blocks, buffer {:.1}MB > limit {:.1}MB, chunking {} rows/chunk",
            blocks_x,
            blocks_y,
            input_bytes as f64 / 1024.0 / 1024.0,
            max_buffer as f64 / 1024.0 / 1024.0,
            max_rows_per_chunk,
        );

        let mut all_coeffs = Vec::with_capacity((total_blocks * 64) as usize);
        let mut row_offset: u32 = 0;

        while row_offset < blocks_y {
            let chunk_rows = max_rows_per_chunk.min(blocks_y - row_offset);
            let chunk_blocks = (blocks_x as usize) * (chunk_rows as usize);
            let data_start = (row_offset as usize) * (blocks_x as usize) * 64;
            let data_end = data_start + chunk_blocks * 64;
            let chunk_data = &channel_data[data_start..data_end];

            let chunk_coeffs =
                self.gpu_dct_quantize_single(chunk_data, blocks_x, chunk_rows, quant_table)?;
            all_coeffs.extend_from_slice(&chunk_coeffs);

            row_offset += chunk_rows;
        }

        Ok(all_coeffs)
    }

    /// 单次 GPU dispatch（不分块）
    fn gpu_dct_quantize_single(
        &self,
        channel_data: &[f32],
        blocks_x: u32,
        blocks_y: u32,
        quant_table: &[f32; 64],
    ) -> Result<Vec<i32>> {
        let device = &self.device;
        let queue = &self.queue;
        let cached = self.get_pipeline();

        let total_blocks = blocks_x * blocks_y;

        // 输入 buffer
        let input_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("jpeg_input"),
            contents: bytemuck::cast_slice(channel_data),
            usage: wgpu::BufferUsages::STORAGE,
        });

        // 输出 buffer: total_blocks * 64 个 i32
        let output_size = (total_blocks as u64) * 64 * 4;
        let output_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("jpeg_output"),
            size: output_size,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        let staging_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("jpeg_staging"),
            size: output_size,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // 量化表 buffer
        let quant_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("jpeg_quant"),
            contents: bytemuck::cast_slice(quant_table),
            usage: wgpu::BufferUsages::STORAGE,
        });

        // 参数 buffer
        let params = JpegDctParams {
            blocks_x,
            blocks_y,
            total_blocks,
            _pad: 0,
        };
        let params_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("jpeg_params"),
            contents: bytemuck::bytes_of(&params),
            usage: wgpu::BufferUsages::UNIFORM,
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("jpeg_bind_group"),
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
                    resource: quant_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: params_buffer.as_entire_binding(),
                },
            ],
        });

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("jpeg_encoder"),
        });

        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("jpeg_dct_pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&cached.pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            // 每个工作组处理一个 8x8 块
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
        let result: Vec<i32> = bytemuck::cast_slice(&mapped).to_vec();
        drop(mapped);
        staging_buffer.unmap();

        Ok(result)
    }
}

impl Encoder for GpuJpegEncoder {
    fn encode(&self, image: &RawImage, params: &EncodeParams) -> Result<EncodedOutput> {
        let EncodeParams::Jpeg {
            quality,
            progressive,
        } = params
        else {
            bail!("GpuJpegEncoder requires Jpeg params");
        };

        let rgb = image.pixels.to_rgb8();
        let (width, height) = rgb.dimensions();

        // === CPU: RGB → YCbCr ===
        let pixel_count = (width * height) as usize;
        let mut y_plane = Vec::with_capacity(pixel_count);
        let mut cb_plane = Vec::with_capacity(pixel_count);
        let mut cr_plane = Vec::with_capacity(pixel_count);

        for p in rgb.pixels() {
            let r = p[0] as f32;
            let g = p[1] as f32;
            let b = p[2] as f32;
            y_plane.push(0.299 * r + 0.587 * g + 0.114 * b);
            cb_plane.push(128.0 - 0.168736 * r - 0.331264 * g + 0.5 * b);
            cr_plane.push(128.0 + 0.5 * r - 0.418688 * g - 0.081312 * b);
        }

        // === CPU: 4:2:0 色度下采样 ===
        let cb_sub = downsample_420(&cb_plane, width, height);
        let cr_sub = downsample_420(&cr_plane, width, height);
        let chroma_w = (width + 1) / 2;
        let chroma_h = (height + 1) / 2;

        // === CPU: Pad to 8x8 boundary ===
        let blocks_x_y = (width + 7) / 8;
        let blocks_y_y = (height + 7) / 8;
        let padded_y = pad_to_blocks(&y_plane, width, height, blocks_x_y, blocks_y_y);

        let blocks_x_c = (chroma_w + 7) / 8;
        let blocks_y_c = (chroma_h + 7) / 8;
        let padded_cb = pad_to_blocks(&cb_sub, chroma_w, chroma_h, blocks_x_c, blocks_y_c);
        let padded_cr = pad_to_blocks(&cr_sub, chroma_w, chroma_h, blocks_x_c, blocks_y_c);

        // === 构建量化表（按 quality 缩放） ===
        let luma_qt = scale_quant_table(&LUMA_QUANT_TABLE, *quality);
        let chroma_qt = scale_quant_table(&CHROMA_QUANT_TABLE, *quality);

        // === GPU: DCT + 量化 + Zigzag ===
        let y_coeffs = self.gpu_dct_quantize(&padded_y, blocks_x_y, blocks_y_y, &luma_qt)?;
        let cb_coeffs = self.gpu_dct_quantize(&padded_cb, blocks_x_c, blocks_y_c, &chroma_qt)?;
        let cr_coeffs = self.gpu_dct_quantize(&padded_cr, blocks_x_c, blocks_y_c, &chroma_qt)?;

        // === CPU: Huffman 编码 + JFIF 组装 ===
        let jpeg_data = assemble_jpeg(
            width,
            height,
            *quality,
            *progressive,
            &y_coeffs,
            &cb_coeffs,
            &cr_coeffs,
            blocks_x_y,
            blocks_y_y,
        )?;

        Ok(EncodedOutput {
            data: jpeg_data,
        })
    }
}

// ============ 辅助函数 ============

/// 4:2:0 色度下采样（2x2 平均）
fn downsample_420(plane: &[f32], width: u32, height: u32) -> Vec<f32> {
    let cw = (width + 1) / 2;
    let ch = (height + 1) / 2;
    let mut out = vec![0.0f32; (cw * ch) as usize];

    for cy in 0..ch {
        for cx in 0..cw {
            let x0 = cx * 2;
            let y0 = cy * 2;
            let mut sum = 0.0f32;
            let mut count = 0u32;
            for dy in 0..2u32 {
                for dx in 0..2u32 {
                    let sx = x0 + dx;
                    let sy = y0 + dy;
                    if sx < width && sy < height {
                        sum += plane[(sy * width + sx) as usize];
                        count += 1;
                    }
                }
            }
            out[(cy * cw + cx) as usize] = sum / count as f32;
        }
    }
    out
}

/// 填充到 8x8 块边界（边缘复制填充）
fn pad_to_blocks(
    data: &[f32],
    width: u32,
    height: u32,
    blocks_x: u32,
    blocks_y: u32,
) -> Vec<f32> {
    let pw = blocks_x * 8;
    let ph = blocks_y * 8;
    let mut padded = vec![0.0f32; (pw * ph) as usize];

    for y in 0..ph {
        for x in 0..pw {
            let sx = x.min(width - 1);
            let sy = y.min(height - 1);
            padded[(y * pw + x) as usize] = data[(sy * width + sx) as usize];
        }
    }
    padded
}

// ============ JPEG 标准量化表 ============

/// JPEG 标准亮度量化表（Annex K, Table K.1）
const LUMA_QUANT_TABLE: [u8; 64] = [
    16, 11, 10, 16, 24, 40, 51, 61, 12, 12, 14, 19, 26, 58, 60, 55, 14, 13, 16, 24, 40, 57, 69,
    56, 14, 17, 22, 29, 51, 87, 80, 62, 18, 22, 37, 56, 68, 109, 103, 77, 24, 35, 55, 64, 81,
    104, 113, 92, 49, 64, 78, 87, 103, 121, 120, 101, 72, 92, 95, 98, 112, 100, 103, 99,
];

/// JPEG 标准色度量化表（Annex K, Table K.2）
const CHROMA_QUANT_TABLE: [u8; 64] = [
    17, 18, 24, 47, 99, 99, 99, 99, 18, 21, 26, 66, 99, 99, 99, 99, 24, 26, 56, 99, 99, 99, 99,
    99, 47, 66, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99,
    99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99,
];

/// 按 JPEG quality (1-100) 缩放量化表
fn scale_quant_table(base: &[u8; 64], quality: u8) -> [f32; 64] {
    let q = quality.clamp(1, 100) as u32;
    let scale = if q < 50 { 5000 / q } else { 200 - q * 2 };

    let mut table = [0.0f32; 64];
    for i in 0..64 {
        let val = ((base[i] as u32 * scale + 50) / 100).clamp(1, 255);
        table[i] = val as f32;
    }
    table
}

/// Zigzag 顺序（用于量化表写入 JFIF）
const ZIGZAG_ORDER: [u8; 64] = [
    0, 1, 8, 16, 9, 2, 3, 10, 17, 24, 32, 25, 18, 11, 4, 5, 12, 19, 26, 33, 40, 48, 41, 34, 27,
    20, 13, 6, 7, 14, 21, 28, 35, 42, 49, 56, 57, 50, 43, 36, 29, 22, 15, 23, 30, 37, 44, 51,
    58, 59, 52, 45, 38, 31, 39, 46, 53, 60, 61, 54, 47, 55, 62, 63,
];

// ============ JFIF 组装 + Huffman 编码 ============

/// 组装完整的 JPEG 文件
fn assemble_jpeg(
    width: u32,
    height: u32,
    quality: u8,
    progressive: bool,
    y_coeffs: &[i32],
    cb_coeffs: &[i32],
    cr_coeffs: &[i32],
    blocks_x: u32,
    blocks_y: u32,
) -> Result<Vec<u8>> {
    let mut out = Vec::with_capacity(width as usize * height as usize);

    // SOI
    out.extend_from_slice(&[0xFF, 0xD8]);

    // APP0 JFIF header
    write_app0(&mut out);

    // DQT (量化表)
    let luma_qt = scale_quant_table(&LUMA_QUANT_TABLE, quality);
    let chroma_qt = scale_quant_table(&CHROMA_QUANT_TABLE, quality);
    write_dqt(&mut out, 0, &luma_qt);
    write_dqt(&mut out, 1, &chroma_qt);

    if progressive {
        // SOF2 (Progressive DCT)
        write_sof2(&mut out, width as u16, height as u16);

        // DHT (Huffman 表)
        write_dht_tables(&mut out);

        // Progressive scans: DC 先行 → AC 逐步填充
        write_progressive_scans(
            &mut out, y_coeffs, cb_coeffs, cr_coeffs, blocks_x, blocks_y,
        )?;
    } else {
        // SOF0 (Baseline DCT)
        write_sof0(&mut out, width as u16, height as u16);

        // DHT (Huffman 表)
        write_dht_tables(&mut out);

        // SOS (Start of Scan) + 扫描数据
        write_sos_and_data(&mut out, y_coeffs, cb_coeffs, cr_coeffs, blocks_x, blocks_y)?;
    }

    // EOI
    out.extend_from_slice(&[0xFF, 0xD9]);

    Ok(out)
}

fn write_app0(out: &mut Vec<u8>) {
    out.extend_from_slice(&[0xFF, 0xE0]); // APP0 marker
    let len: u16 = 16;
    out.extend_from_slice(&len.to_be_bytes());
    out.extend_from_slice(b"JFIF\0"); // identifier
    out.extend_from_slice(&[1, 1]); // version 1.1
    out.push(0); // density units: no units
    out.extend_from_slice(&1u16.to_be_bytes()); // X density
    out.extend_from_slice(&1u16.to_be_bytes()); // Y density
    out.extend_from_slice(&[0, 0]); // thumbnail size
}

fn write_dqt(out: &mut Vec<u8>, table_id: u8, table: &[f32; 64]) {
    out.extend_from_slice(&[0xFF, 0xDB]); // DQT marker
    let len: u16 = 67; // 2 + 1 + 64
    out.extend_from_slice(&len.to_be_bytes());
    out.push(table_id); // 8-bit precision (0) | table ID
    for i in 0..64 {
        out.push(table[ZIGZAG_ORDER[i] as usize] as u8);
    }
}

fn write_sof0(out: &mut Vec<u8>, width: u16, height: u16) {
    out.extend_from_slice(&[0xFF, 0xC0]); // SOF0 marker
    let len: u16 = 17; // 2 + 1 + 2 + 2 + 1 + 3*3
    out.extend_from_slice(&len.to_be_bytes());
    out.push(8); // 8-bit precision
    out.extend_from_slice(&height.to_be_bytes());
    out.extend_from_slice(&width.to_be_bytes());
    out.push(3); // 3 components (YCbCr)
    // Component 1 (Y): ID=1, sampling=2x2, quant table=0
    out.extend_from_slice(&[1, 0x22, 0]);
    // Component 2 (Cb): ID=2, sampling=1x1, quant table=1
    out.extend_from_slice(&[2, 0x11, 1]);
    // Component 3 (Cr): ID=3, sampling=1x1, quant table=1
    out.extend_from_slice(&[3, 0x11, 1]);
}

fn write_sof2(out: &mut Vec<u8>, width: u16, height: u16) {
    out.extend_from_slice(&[0xFF, 0xC2]); // SOF2 progressive marker
    let len: u16 = 17;
    out.extend_from_slice(&len.to_be_bytes());
    out.push(8); // 8-bit precision
    out.extend_from_slice(&height.to_be_bytes());
    out.extend_from_slice(&width.to_be_bytes());
    out.push(3); // 3 components
    out.extend_from_slice(&[1, 0x22, 0]); // Y: 2x2, quant 0
    out.extend_from_slice(&[2, 0x11, 1]); // Cb: 1x1, quant 1
    out.extend_from_slice(&[3, 0x11, 1]); // Cr: 1x1, quant 1
}

/// Progressive JPEG 扫描方案:
///   Scan 0: DC all (Y+Cb+Cr interleaved)  — 快速显示模糊缩略图
///   Scan 1: Y AC 1-5                       — 低频细节
///   Scan 2: Y AC 6-63                      — 高频细节
///   Scan 3: Cb AC 1-63                     — 色度细节
///   Scan 4: Cr AC 1-63                     — 色度细节
fn write_progressive_scans(
    out: &mut Vec<u8>,
    y_coeffs: &[i32],
    cb_coeffs: &[i32],
    cr_coeffs: &[i32],
    blocks_x: u32,
    blocks_y: u32,
) -> Result<()> {
    let dc_luma = HuffTable::build(&DC_LUMA_BITS, &DC_LUMA_VALS);
    let ac_luma = HuffTable::build(&AC_LUMA_BITS, &AC_LUMA_VALS);
    let dc_chroma = HuffTable::build(&DC_CHROMA_BITS, &DC_CHROMA_VALS);
    let ac_chroma = HuffTable::build(&AC_CHROMA_BITS, &AC_CHROMA_VALS);

    let chroma_blocks_x = (blocks_x + 1) / 2;
    let chroma_blocks_y = (blocks_y + 1) / 2;
    let mcu_x = (blocks_x + 1) / 2;
    let mcu_y = (blocks_y + 1) / 2;

    // === Scan 0: DC all components (interleaved) ===
    {
        // SOS header: 3 components, Ss=0, Se=0
        write_sos_header(out, &[(1, 0x00), (2, 0x11), (3, 0x11)], 0, 0, 0);

        let mut writer = BitWriter::new();
        let mut dc_y = 0i32;
        let mut dc_cb = 0i32;
        let mut dc_cr = 0i32;

        for mcu_row in 0..mcu_y {
            for mcu_col in 0..mcu_x {
                // 4 Y DC
                for dy in 0..2u32 {
                    for dx in 0..2u32 {
                        let bx = mcu_col * 2 + dx;
                        let by = mcu_row * 2 + dy;
                        let dc_val = if bx < blocks_x && by < blocks_y {
                            y_coeffs[((by * blocks_x + bx) * 64) as usize]
                        } else {
                            dc_y // repeat last DC for out-of-bounds
                        };
                        encode_dc(&mut writer, dc_val, &mut dc_y, &dc_luma);
                    }
                }
                // 1 Cb DC
                let cb_idx = (mcu_row * chroma_blocks_x + mcu_col) as usize;
                let cb_dc = if cb_idx * 64 < cb_coeffs.len() {
                    cb_coeffs[cb_idx * 64]
                } else {
                    dc_cb
                };
                encode_dc(&mut writer, cb_dc, &mut dc_cb, &dc_chroma);
                // 1 Cr DC
                let cr_dc = if cb_idx * 64 < cr_coeffs.len() {
                    cr_coeffs[cb_idx * 64]
                } else {
                    dc_cr
                };
                encode_dc(&mut writer, cr_dc, &mut dc_cr, &dc_chroma);
            }
        }
        writer.flush();
        out.extend_from_slice(&writer.data);
    }

    // === Scan 1: Y AC 1-5 (non-interleaved) ===
    write_progressive_ac_scan(
        out, y_coeffs, blocks_x, blocks_y, 1, 5, &ac_luma, 1, 0x00,
    );

    // === Scan 2: Y AC 6-63 (non-interleaved) ===
    write_progressive_ac_scan(
        out, y_coeffs, blocks_x, blocks_y, 6, 63, &ac_luma, 1, 0x00,
    );

    // === Scan 3: Cb AC 1-63 (non-interleaved) ===
    write_progressive_ac_scan(
        out,
        cb_coeffs,
        chroma_blocks_x,
        chroma_blocks_y,
        1,
        63,
        &ac_chroma,
        2,
        0x11,
    );

    // === Scan 4: Cr AC 1-63 (non-interleaved) ===
    write_progressive_ac_scan(
        out,
        cr_coeffs,
        chroma_blocks_x,
        chroma_blocks_y,
        1,
        63,
        &ac_chroma,
        3,
        0x11,
    );

    Ok(())
}

/// 写入 SOS header
fn write_sos_header(
    out: &mut Vec<u8>,
    components: &[(u8, u8)], // (component_id, dc_ac_table_selector)
    ss: u8,                  // spectral selection start
    se: u8,                  // spectral selection end
    ahl: u8,                 // successive approximation (Ah << 4 | Al)
) {
    out.extend_from_slice(&[0xFF, 0xDA]);
    let len: u16 = 2 + 1 + (components.len() as u16) * 2 + 3;
    out.extend_from_slice(&len.to_be_bytes());
    out.push(components.len() as u8);
    for &(id, selector) in components {
        out.push(id);
        out.push(selector);
    }
    out.push(ss);
    out.push(se);
    out.push(ahl);
}

/// 编码一个 DC 系数（差分编码）
fn encode_dc(writer: &mut BitWriter, dc_val: i32, prev_dc: &mut i32, dc_table: &HuffTable) {
    let dc_diff = dc_val - *prev_dc;
    *prev_dc = dc_val;
    let (category, extra) = encode_value(dc_diff);
    writer.write_bits(dc_table.code[category as usize], dc_table.size[category as usize]);
    if category > 0 {
        writer.write_bits(extra, category);
    }
}

/// 写入 progressive AC 扫描（非交织，单组件）
fn write_progressive_ac_scan(
    out: &mut Vec<u8>,
    coeffs: &[i32],
    blocks_x: u32,
    blocks_y: u32,
    ss: u8,  // start of spectral selection
    se: u8,  // end of spectral selection
    ac_table: &HuffTable,
    comp_id: u8,
    table_selector: u8,
) {
    write_sos_header(out, &[(comp_id, table_selector)], ss, se, 0);

    let mut writer = BitWriter::new();
    let total_blocks = blocks_x * blocks_y;

    for block in 0..total_blocks {
        let base = (block * 64) as usize;
        if base + 64 > coeffs.len() {
            // 超出范围写零块
            let eob_symbol = 0x00u8;
            writer.write_bits(ac_table.code[eob_symbol as usize], ac_table.size[eob_symbol as usize]);
            continue;
        }

        let block_coeffs = &coeffs[base..base + 64];
        let mut zero_run = 0u8;
        let mut wrote_nonzero = false;

        for i in (ss as usize)..=(se as usize) {
            if block_coeffs[i] == 0 {
                zero_run += 1;
            } else {
                while zero_run >= 16 {
                    writer.write_bits(ac_table.code[0xF0], ac_table.size[0xF0]);
                    zero_run -= 16;
                }
                let (category, extra) = encode_value(block_coeffs[i]);
                let symbol = (zero_run << 4) | category;
                writer.write_bits(
                    ac_table.code[symbol as usize],
                    ac_table.size[symbol as usize],
                );
                writer.write_bits(extra, category);
                zero_run = 0;
                wrote_nonzero = true;
            }
        }

        // EOB
        if zero_run > 0 || !wrote_nonzero {
            writer.write_bits(ac_table.code[0x00], ac_table.size[0x00]);
        }
    }

    writer.flush();
    out.extend_from_slice(&writer.data);
}

// ============ 标准 JPEG Huffman 表 (Annex K) ============

/// DC 亮度 Huffman 表 bits
const DC_LUMA_BITS: [u8; 16] = [0, 1, 5, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0];
const DC_LUMA_VALS: [u8; 12] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11];

/// DC 色度 Huffman 表
const DC_CHROMA_BITS: [u8; 16] = [0, 3, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0];
const DC_CHROMA_VALS: [u8; 12] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11];

/// AC 亮度 Huffman 表
const AC_LUMA_BITS: [u8; 16] = [0, 2, 1, 3, 3, 2, 4, 3, 5, 5, 4, 4, 0, 0, 1, 125];
const AC_LUMA_VALS: [u8; 162] = [
    0x01, 0x02, 0x03, 0x00, 0x04, 0x11, 0x05, 0x12, 0x21, 0x31, 0x41, 0x06, 0x13, 0x51, 0x61,
    0x07, 0x22, 0x71, 0x14, 0x32, 0x81, 0x91, 0xA1, 0x08, 0x23, 0x42, 0xB1, 0xC1, 0x15, 0x52,
    0xD1, 0xF0, 0x24, 0x33, 0x62, 0x72, 0x82, 0x09, 0x0A, 0x16, 0x17, 0x18, 0x19, 0x1A, 0x25,
    0x26, 0x27, 0x28, 0x29, 0x2A, 0x34, 0x35, 0x36, 0x37, 0x38, 0x39, 0x3A, 0x43, 0x44, 0x45,
    0x46, 0x47, 0x48, 0x49, 0x4A, 0x53, 0x54, 0x55, 0x56, 0x57, 0x58, 0x59, 0x5A, 0x63, 0x64,
    0x65, 0x66, 0x67, 0x68, 0x69, 0x6A, 0x73, 0x74, 0x75, 0x76, 0x77, 0x78, 0x79, 0x7A, 0x83,
    0x84, 0x85, 0x86, 0x87, 0x88, 0x89, 0x8A, 0x92, 0x93, 0x94, 0x95, 0x96, 0x97, 0x98, 0x99,
    0x9A, 0xA2, 0xA3, 0xA4, 0xA5, 0xA6, 0xA7, 0xA8, 0xA9, 0xAA, 0xB2, 0xB3, 0xB4, 0xB5, 0xB6,
    0xB7, 0xB8, 0xB9, 0xBA, 0xC2, 0xC3, 0xC4, 0xC5, 0xC6, 0xC7, 0xC8, 0xC9, 0xCA, 0xD2, 0xD3,
    0xD4, 0xD5, 0xD6, 0xD7, 0xD8, 0xD9, 0xDA, 0xE1, 0xE2, 0xE3, 0xE4, 0xE5, 0xE6, 0xE7, 0xE8,
    0xE9, 0xEA, 0xF1, 0xF2, 0xF3, 0xF4, 0xF5, 0xF6, 0xF7, 0xF8, 0xF9, 0xFA,
];

/// AC 色度 Huffman 表
const AC_CHROMA_BITS: [u8; 16] = [0, 2, 1, 2, 4, 4, 3, 4, 7, 5, 4, 4, 0, 1, 2, 119];
const AC_CHROMA_VALS: [u8; 162] = [
    0x00, 0x01, 0x02, 0x03, 0x11, 0x04, 0x05, 0x21, 0x31, 0x06, 0x12, 0x41, 0x51, 0x07, 0x61,
    0x71, 0x13, 0x22, 0x32, 0x81, 0x08, 0x14, 0x42, 0x91, 0xA1, 0xB1, 0xC1, 0x09, 0x23, 0x33,
    0x52, 0xF0, 0x15, 0x62, 0x72, 0xD1, 0x0A, 0x16, 0x24, 0x34, 0xE1, 0x25, 0xF1, 0x17, 0x18,
    0x19, 0x1A, 0x26, 0x27, 0x28, 0x29, 0x2A, 0x35, 0x36, 0x37, 0x38, 0x39, 0x3A, 0x43, 0x44,
    0x45, 0x46, 0x47, 0x48, 0x49, 0x4A, 0x53, 0x54, 0x55, 0x56, 0x57, 0x58, 0x59, 0x5A, 0x63,
    0x64, 0x65, 0x66, 0x67, 0x68, 0x69, 0x6A, 0x73, 0x74, 0x75, 0x76, 0x77, 0x78, 0x79, 0x7A,
    0x82, 0x83, 0x84, 0x85, 0x86, 0x87, 0x88, 0x89, 0x8A, 0x92, 0x93, 0x94, 0x95, 0x96, 0x97,
    0x98, 0x99, 0x9A, 0xA2, 0xA3, 0xA4, 0xA5, 0xA6, 0xA7, 0xA8, 0xA9, 0xAA, 0xB2, 0xB3, 0xB4,
    0xB5, 0xB6, 0xB7, 0xB8, 0xB9, 0xBA, 0xC2, 0xC3, 0xC4, 0xC5, 0xC6, 0xC7, 0xC8, 0xC9, 0xCA,
    0xD2, 0xD3, 0xD4, 0xD5, 0xD6, 0xD7, 0xD8, 0xD9, 0xDA, 0xE2, 0xE3, 0xE4, 0xE5, 0xE6, 0xE7,
    0xE8, 0xE9, 0xEA, 0xF2, 0xF3, 0xF4, 0xF5, 0xF6, 0xF7, 0xF8, 0xF9, 0xFA,
];

/// 从 bits/vals 构建 Huffman 编码查找表
struct HuffTable {
    /// code[symbol] = (huffman_code, bit_length)
    code: [u32; 256],
    size: [u8; 256],
}

impl HuffTable {
    fn build(bits: &[u8; 16], vals: &[u8]) -> Self {
        let mut code_table = [0u32; 256];
        let mut size_table = [0u8; 256];

        let mut code: u32 = 0;
        let mut si = 0usize;
        for (i, &count) in bits.iter().enumerate() {
            let bit_len = (i + 1) as u8;
            for _ in 0..count {
                let symbol = vals[si] as usize;
                code_table[symbol] = code;
                size_table[symbol] = bit_len;
                code += 1;
                si += 1;
            }
            code <<= 1;
        }

        HuffTable {
            code: code_table,
            size: size_table,
        }
    }
}

/// Bitstream 写入器
struct BitWriter {
    data: Vec<u8>,
    bit_buf: u32,
    bit_count: u8,
}

impl BitWriter {
    fn new() -> Self {
        Self {
            data: Vec::new(),
            bit_buf: 0,
            bit_count: 0,
        }
    }

    fn write_bits(&mut self, value: u32, nbits: u8) {
        self.bit_buf = (self.bit_buf << nbits) | (value & ((1 << nbits) - 1));
        self.bit_count += nbits;

        while self.bit_count >= 8 {
            self.bit_count -= 8;
            let byte = (self.bit_buf >> self.bit_count) as u8;
            self.data.push(byte);
            // Byte stuffing: 0xFF 后插入 0x00
            if byte == 0xFF {
                self.data.push(0x00);
            }
        }
    }

    fn flush(&mut self) {
        if self.bit_count > 0 {
            let byte = (self.bit_buf << (8 - self.bit_count)) as u8;
            self.data.push(byte);
            if byte == 0xFF {
                self.data.push(0x00);
            }
            self.bit_count = 0;
            self.bit_buf = 0;
        }
    }
}

fn write_dht(out: &mut Vec<u8>, class: u8, id: u8, bits: &[u8; 16], vals: &[u8]) {
    out.extend_from_slice(&[0xFF, 0xC4]); // DHT marker
    let total_vals: u16 = bits.iter().map(|&b| b as u16).sum();
    let len: u16 = 2 + 1 + 16 + total_vals;
    out.extend_from_slice(&len.to_be_bytes());
    out.push((class << 4) | id);
    out.extend_from_slice(bits);
    out.extend_from_slice(&vals[..total_vals as usize]);
}

fn write_dht_tables(out: &mut Vec<u8>) {
    write_dht(out, 0, 0, &DC_LUMA_BITS, &DC_LUMA_VALS); // DC luma
    write_dht(out, 1, 0, &AC_LUMA_BITS, &AC_LUMA_VALS); // AC luma
    write_dht(out, 0, 1, &DC_CHROMA_BITS, &DC_CHROMA_VALS); // DC chroma
    write_dht(out, 1, 1, &AC_CHROMA_BITS, &AC_CHROMA_VALS); // AC chroma
}

/// 计算一个值的位类别 (category) 和额外位
fn encode_value(value: i32) -> (u8, u32) {
    if value == 0 {
        return (0, 0);
    }
    let abs = value.unsigned_abs();
    let category = 32 - abs.leading_zeros(); // ceil(log2(abs+1))
    let extra = if value > 0 {
        value as u32
    } else {
        (value - 1) as u32 // one's complement for negative
    };
    (category as u8, extra & ((1 << category) - 1))
}

/// 编码一个 8x8 块的 DC+AC 系数
fn encode_block(
    writer: &mut BitWriter,
    coeffs: &[i32],
    prev_dc: &mut i32,
    dc_table: &HuffTable,
    ac_table: &HuffTable,
) {
    // DC: 差分编码
    let dc_diff = coeffs[0] - *prev_dc;
    *prev_dc = coeffs[0];

    let (category, extra) = encode_value(dc_diff);
    writer.write_bits(dc_table.code[category as usize], dc_table.size[category as usize]);
    if category > 0 {
        writer.write_bits(extra, category);
    }

    // AC: 游程编码
    let mut zero_run = 0u8;
    for i in 1..64 {
        if coeffs[i] == 0 {
            zero_run += 1;
        } else {
            // 输出 ZRL (16 个连续零) 符号
            while zero_run >= 16 {
                writer.write_bits(ac_table.code[0xF0], ac_table.size[0xF0]);
                zero_run -= 16;
            }
            let (category, extra) = encode_value(coeffs[i]);
            let symbol = (zero_run << 4) | category;
            writer.write_bits(
                ac_table.code[symbol as usize],
                ac_table.size[symbol as usize],
            );
            writer.write_bits(extra, category);
            zero_run = 0;
        }
    }

    // EOB (End of Block) = (0, 0)
    if zero_run > 0 {
        writer.write_bits(ac_table.code[0x00], ac_table.size[0x00]);
    }
}

fn write_sos_and_data(
    out: &mut Vec<u8>,
    y_coeffs: &[i32],
    cb_coeffs: &[i32],
    cr_coeffs: &[i32],
    blocks_x: u32,
    blocks_y: u32,
) -> Result<()> {
    // SOS header
    out.extend_from_slice(&[0xFF, 0xDA]); // SOS marker
    let len: u16 = 12; // 2 + 1 + 3*2 + 3
    out.extend_from_slice(&len.to_be_bytes());
    out.push(3); // 3 components
    out.extend_from_slice(&[1, 0x00]); // Y: DC table 0, AC table 0
    out.extend_from_slice(&[2, 0x11]); // Cb: DC table 1, AC table 1
    out.extend_from_slice(&[3, 0x11]); // Cr: DC table 1, AC table 1
    out.extend_from_slice(&[0, 63, 0]); // spectral selection: 0-63, successive approx: 0

    // 构建 Huffman 表
    let dc_luma = HuffTable::build(&DC_LUMA_BITS, &DC_LUMA_VALS);
    let ac_luma = HuffTable::build(&AC_LUMA_BITS, &AC_LUMA_VALS);
    let dc_chroma = HuffTable::build(&DC_CHROMA_BITS, &DC_CHROMA_VALS);
    let ac_chroma = HuffTable::build(&AC_CHROMA_BITS, &AC_CHROMA_VALS);

    let mut writer = BitWriter::new();
    let mut dc_y = 0i32;
    let mut dc_cb = 0i32;
    let mut dc_cr = 0i32;

    // 色度块数
    let chroma_blocks_x = (blocks_x + 1) / 2;

    // 按 MCU（Minimum Coded Unit）顺序: 4:2:0 → 每个 MCU = 4Y + 1Cb + 1Cr
    let mcu_x = (blocks_x + 1) / 2;
    let mcu_y = (blocks_y + 1) / 2;

    for mcu_row in 0..mcu_y {
        for mcu_col in 0..mcu_x {
            // 4 个 Y 块 (2x2)
            for dy in 0..2u32 {
                for dx in 0..2u32 {
                    let bx = mcu_col * 2 + dx;
                    let by = mcu_row * 2 + dy;
                    if bx < blocks_x && by < blocks_y {
                        let idx = ((by * blocks_x + bx) * 64) as usize;
                        encode_block(
                            &mut writer,
                            &y_coeffs[idx..idx + 64],
                            &mut dc_y,
                            &dc_luma,
                            &ac_luma,
                        );
                    } else {
                        // 超出范围的块编码为零
                        let zeros = [0i32; 64];
                        encode_block(&mut writer, &zeros, &mut dc_y, &dc_luma, &ac_luma);
                    }
                }
            }

            // 1 个 Cb 块
            {
                let idx = ((mcu_row * chroma_blocks_x + mcu_col) * 64) as usize;
                if idx + 64 <= cb_coeffs.len() {
                    encode_block(
                        &mut writer,
                        &cb_coeffs[idx..idx + 64],
                        &mut dc_cb,
                        &dc_chroma,
                        &ac_chroma,
                    );
                } else {
                    let zeros = [0i32; 64];
                    encode_block(&mut writer, &zeros, &mut dc_cb, &dc_chroma, &ac_chroma);
                }
            }

            // 1 个 Cr 块
            {
                let idx = ((mcu_row * chroma_blocks_x + mcu_col) * 64) as usize;
                if idx + 64 <= cr_coeffs.len() {
                    encode_block(
                        &mut writer,
                        &cr_coeffs[idx..idx + 64],
                        &mut dc_cr,
                        &dc_chroma,
                        &ac_chroma,
                    );
                } else {
                    let zeros = [0i32; 64];
                    encode_block(&mut writer, &zeros, &mut dc_cr, &dc_chroma, &ac_chroma);
                }
            }
        }
    }

    writer.flush();
    out.extend_from_slice(&writer.data);

    Ok(())
}

// ============ WGSL Shader ============

/// 合并 DCT + 量化 + Zigzag 重排 的 compute shader
///
/// 每个工作组 (8x8=64 线程) 处理一个 8x8 块。
/// 输入: 按图像行排列的 f32 像素 (0~255)
/// 输出: 每块 64 个 i32 量化系数（zigzag 顺序），按块索引连续排列
const JPEG_DCT_QUANTIZE_SHADER: &str = r#"
struct Params {
    blocks_x: u32,
    blocks_y: u32,
    total_blocks: u32,
    _pad: u32,
}

@group(0) @binding(0) var<storage, read> input: array<f32>;
@group(0) @binding(1) var<storage, read_write> output: array<i32>;
@group(0) @binding(2) var<storage, read> quant_table: array<f32>;
@group(0) @binding(3) var<uniform> params: Params;

const PI: f32 = 3.14159265358979323846;

// Zigzag 顺序查找表
const ZIGZAG: array<u32, 64> = array<u32, 64>(
     0u,  1u,  8u, 16u,  9u,  2u,  3u, 10u,
    17u, 24u, 32u, 25u, 18u, 11u,  4u,  5u,
    12u, 19u, 26u, 33u, 40u, 48u, 41u, 34u,
    27u, 20u, 13u,  6u,  7u, 14u, 21u, 28u,
    35u, 42u, 49u, 56u, 57u, 50u, 43u, 36u,
    29u, 22u, 15u, 23u, 30u, 37u, 44u, 51u,
    58u, 59u, 52u, 45u, 38u, 31u, 39u, 46u,
    53u, 60u, 61u, 54u, 47u, 55u, 62u, 63u
);

// 反向 Zigzag: ZIGZAG[i] = natural_pos → zigzag_pos[natural_pos] = i
// 由于 WGSL 不支持初始化时反转，我们直接遍历 ZIGZAG 做映射

fn alpha(k: u32) -> f32 {
    if (k == 0u) {
        return sqrt(1.0 / 8.0);
    } else {
        return sqrt(2.0 / 8.0);
    }
}

var<workgroup> pixels: array<f32, 64>;   // 原始像素 (level-shifted)
var<workgroup> temp: array<f32, 64>;     // 行 DCT 结果

@compute @workgroup_size(8, 8)
fn main(
    @builtin(local_invocation_id) lid: vec3<u32>,
    @builtin(workgroup_id) wid: vec3<u32>,
) {
    let block_idx = wid.y * params.blocks_x + wid.x;
    let local_idx = lid.y * 8u + lid.x;
    let padded_w = params.blocks_x * 8u;

    // 读取像素到共享内存（减 128 居中）
    let px = wid.x * 8u + lid.x;
    let py = wid.y * 8u + lid.y;
    pixels[local_idx] = input[py * padded_w + px] - 128.0;

    workgroupBarrier();

    // === 行 DCT ===
    // lid.y = 行号, lid.x = 频率 u
    var row_sum: f32 = 0.0;
    for (var n = 0u; n < 8u; n = n + 1u) {
        row_sum += pixels[lid.y * 8u + n] * cos(PI * (f32(n) + 0.5) * f32(lid.x) / 8.0);
    }
    temp[local_idx] = row_sum * alpha(lid.x);

    workgroupBarrier();

    // === 列 DCT ===
    // lid.y = 频率 v, lid.x = 列号 (= 频率 u)
    var col_sum: f32 = 0.0;
    for (var m = 0u; m < 8u; m = m + 1u) {
        col_sum += temp[m * 8u + lid.x] * cos(PI * (f32(m) + 0.5) * f32(lid.y) / 8.0);
    }
    let dct_coeff = col_sum * alpha(lid.y);

    // === 量化 ===
    // natural order position: (lid.y, lid.x) → index lid.y*8+lid.x
    let natural_pos = lid.y * 8u + lid.x;
    let quantized = i32(round(dct_coeff / quant_table[natural_pos]));

    // === Zigzag 重排写入 ===
    // 找到 natural_pos 在 zigzag 中的位置
    // 查找: ZIGZAG[zigzag_idx] == natural_pos → 需要反查
    // 为避免循环查找，直接用 natural_pos 在 ZIGZAG 数组中找
    var zigzag_idx: u32 = 0u;
    for (var i = 0u; i < 64u; i = i + 1u) {
        if (ZIGZAG[i] == natural_pos) {
            zigzag_idx = i;
            break;
        }
    }

    // 输出: 按块连续排列, 每块 64 个 i32
    output[block_idx * 64u + zigzag_idx] = quantized;
}
"#;
