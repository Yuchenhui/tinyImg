mod app;
mod config;
mod engine;
mod gpu;
mod i18n;
mod worker;

use app::App;
use engine::codec::universal::UniversalDecoder;
use engine::codec::{Codec, Decoder};
use engine::params::{EncodeParams, ImageFormat};
use engine::pipeline::CompressionPipeline;
use engine::preprocess::metadata::MetadataStripper;
use gpu::context::GpuAccelerator;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use slint::Model;

slint::include_modules!();

fn main() -> anyhow::Result<()> {
    // 初始化日志
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into()),
        )
        .init();

    tracing::info!("TinyImg v{} starting", env!("CARGO_PKG_VERSION"));

    // 初始化应用核心
    let app = App::new()?;

    // 设置 i18n
    i18n::set_language(&app.config.language);

    // 创建 Slint 窗口
    let ui = AppWindow::new()?;

    // 注册 UI 回调
    setup_callbacks(&ui, app);

    // 启动事件循环
    ui.run()?;

    Ok(())
}

/// 获取支持的图片格式文件扩展名
fn supported_extensions() -> Vec<&'static str> {
    vec![
        "jpg", "jpeg", "png", "webp", "avif", "jxl", "gif", "svg", "svgz",
        "bmp", "tiff", "tif", "ico",
    ]
}

/// 检查文件是否是支持的图片格式
fn is_supported_image(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| {
            supported_extensions()
                .iter()
                .any(|&supported| ext.eq_ignore_ascii_case(supported))
        })
        .unwrap_or(false)
}

/// 递归扫描目录获取图片文件
fn scan_images_recursive(dir: &Path) -> Vec<PathBuf> {
    let mut images = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                images.extend(scan_images_recursive(&path));
            } else if is_supported_image(&path) {
                images.push(path);
            }
        }
    }
    images
}

/// 格式化文件大小为人类可读字符串
fn format_size(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    let b = bytes as f64;
    if b < KB {
        format!("{bytes} B")
    } else if b < MB {
        format!("{:.1} KB", b / KB)
    } else {
        format!("{:.2} MB", b / MB)
    }
}

/// 将文件列表添加到 UI 图片列表
fn add_files_to_ui(ui: &AppWindow, paths: Vec<PathBuf>) {
    let state = ui.global::<AppState>();
    let model = state.get_images();

    // 获取当前最大 ID
    let mut next_id = 0i32;
    for i in 0..model.row_count() {
        if let Some(item) = model.row_data(i) {
            next_id = next_id.max(item.id + 1);
        }
    }

    let vec_model = model
        .as_any()
        .downcast_ref::<slint::VecModel<ImageItem>>()
        .expect("images model should be VecModel");

    for path in paths {
        let filename = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        let filepath = path.to_string_lossy().to_string();

        let format = path
            .extension()
            .and_then(|e| e.to_str())
            .and_then(ImageFormat::from_extension)
            .map(|f| f.to_string())
            .unwrap_or_else(|| "Unknown".to_string());

        let size = std::fs::metadata(&path)
            .map(|m| m.len())
            .unwrap_or(0);

        let item = ImageItem {
            id: next_id,
            filename: filename.into(),
            filepath: filepath.into(),
            format: format.into(),
            original_size: format_size(size).into(),
            compressed_size: slint::SharedString::default(),
            compression_ratio: slint::SharedString::default(),
            status: "pending".into(),
            error_message: slint::SharedString::default(),
        };

        vec_model.push(item);
        next_id += 1;
    }

    state.set_total_count(vec_model.row_count() as i32);
}

/// 根据源格式获取默认编码参数
fn default_encode_params(format: ImageFormat) -> EncodeParams {
    match format {
        ImageFormat::Jpeg => EncodeParams::Jpeg {
            quality: 80,
            progressive: true,
        },
        ImageFormat::Png => EncodeParams::Png {
            lossy: false,
            optimization_level: 2,
        },
        ImageFormat::WebP => EncodeParams::WebP {
            quality: 80,
            lossless: true,
        },
        ImageFormat::Avif => EncodeParams::Avif {
            quality: 70,
            speed: 4,
        },
        ImageFormat::Gif => EncodeParams::Gif {
            quality: 80,
            fast: false,
        },
        // JXL/SVG 编码暂不支持，fallback 到 PNG
        _ => EncodeParams::Png {
            lossy: false,
            optimization_level: 2,
        },
    }
}

fn setup_callbacks(ui: &AppWindow, app: App) {
    let bridge = ui.global::<AppBridge>();

    // 共享 GPU 加速器（跨回调使用）
    let gpu = Arc::new(app.gpu);

    // 初始化图片列表 model（确保是 VecModel）
    {
        let state = ui.global::<AppState>();
        let model: slint::VecModel<ImageItem> = slint::VecModel::default();
        state.set_images(slint::ModelRc::new(model));

        // 设置 GPU 状态到 UI
        state.set_gpu_available(gpu.is_available());
        #[cfg(feature = "gpu")]
        {
            state.set_gpu_name(gpu.name().into());
        }
    }

    // ===== 添加文件 =====
    bridge.on_add_files({
        let ui_handle = ui.as_weak();
        move || {
            tracing::info!("Add files requested");
            let ui = ui_handle.unwrap();

            // 构建文件过滤器
            let dialog = rfd::FileDialog::new()
                .set_title("选择图片文件")
                .add_filter(
                    "图片文件",
                    &["jpg", "jpeg", "png", "webp", "avif", "gif", "bmp", "tiff", "ico"],
                )
                .add_filter("所有文件", &["*"]);

            if let Some(paths) = Some(dialog.pick_files()).filter(|p| p.is_some()).flatten() {
                tracing::info!("Selected {} files", paths.len());
                add_files_to_ui(&ui, paths);
            }
        }
    });

    // ===== 添加文件夹 =====
    bridge.on_add_folder({
        let ui_handle = ui.as_weak();
        move || {
            tracing::info!("Add folder requested");
            let ui = ui_handle.unwrap();

            let dialog = rfd::FileDialog::new().set_title("选择文件夹");

            if let Some(folder) = dialog.pick_folder() {
                tracing::info!("Scanning folder: {}", folder.display());
                let images = scan_images_recursive(&folder);
                tracing::info!("Found {} images", images.len());
                add_files_to_ui(&ui, images);
            }
        }
    });

    // ===== 压缩全部 =====
    bridge.on_compress_all({
        let ui_handle = ui.as_weak();
        let cancel_flag = Arc::new(AtomicBool::new(false));
        let cancel_flag_clone = Arc::clone(&cancel_flag);
        let gpu = Arc::clone(&gpu);

        // 注册取消回调
        {
            let bridge = ui.global::<AppBridge>();
            bridge.on_cancel(move || {
                tracing::info!("Cancel requested");
                cancel_flag_clone.store(true, Ordering::Relaxed);
            });
        }

        move || {
            tracing::info!("Compress all requested");
            let ui = ui_handle.unwrap();
            let state = ui.global::<AppState>();

            // 收集待压缩文件
            let model = state.get_images();
            let mut tasks: Vec<(i32, PathBuf)> = Vec::new();

            for i in 0..model.row_count() {
                if let Some(item) = model.row_data(i) {
                    if item.status.as_str() == "pending" {
                        tasks.push((item.id, PathBuf::from(item.filepath.as_str())));
                    }
                }
            }

            if tasks.is_empty() {
                state.set_status_text("没有待压缩的文件".into());
                return;
            }

            let total = tasks.len();
            state.set_status_text(format!("正在压缩 {total} 个文件...").into());
            state.set_completed_count(0);
            state.set_progress(0.0);

            // 重置取消标志
            cancel_flag.store(false, Ordering::Relaxed);
            let cancel = Arc::clone(&cancel_flag);

            // 标记所有任务为 processing
            for i in 0..model.row_count() {
                if let Some(mut item) = model.row_data(i) {
                    if item.status.as_str() == "pending" {
                        item.status = "processing".into();
                        model.set_row_data(i, item);
                    }
                }
            }

            // 在后台线程启动压缩
            let ui_handle = ui.as_weak();
            let completed_count = Arc::new(Mutex::new(0usize));
            let gpu = Arc::clone(&gpu);

            std::thread::spawn(move || {
                use rayon::prelude::*;

                tasks.par_iter().for_each(|(id, input_path)| {
                    if cancel.load(Ordering::Relaxed) {
                        return;
                    }

                    let task_id = *id;
                    let result = compress_single_file(input_path, &gpu);

                    let ui_handle = ui_handle.clone();
                    let completed_count = Arc::clone(&completed_count);

                    // 回到 UI 线程更新状态
                    slint::invoke_from_event_loop(move || {
                        let ui = ui_handle.unwrap();
                        let state = ui.global::<AppState>();
                        let model = state.get_images();

                        // 找到对应的行
                        for i in 0..model.row_count() {
                            if let Some(mut item) = model.row_data(i) {
                                if item.id == task_id {
                                    match &result {
                                        Ok((compressed_size, output_path)) => {
                                            item.status = "completed".into();
                                            item.compressed_size =
                                                format_size(*compressed_size).into();

                                            // 计算压缩率
                                            let original = std::fs::metadata(
                                                PathBuf::from(item.filepath.as_str()),
                                            )
                                            .map(|m| m.len())
                                            .unwrap_or(0);

                                            if original > 0 {
                                                let ratio = 1.0
                                                    - (*compressed_size as f64 / original as f64);
                                                item.compression_ratio =
                                                    format!("{:.1}%", ratio * 100.0).into();
                                            }

                                            tracing::info!(
                                                "Compressed: {} -> {}",
                                                item.filename,
                                                output_path.display()
                                            );
                                        }
                                        Err(e) => {
                                            item.status = "failed".into();
                                            item.error_message = e.to_string().into();
                                            tracing::error!(
                                                "Failed to compress {}: {e}",
                                                item.filename
                                            );
                                        }
                                    }
                                    model.set_row_data(i, item);
                                    break;
                                }
                            }
                        }

                        // 更新进度
                        let mut count = completed_count.lock().unwrap();
                        *count += 1;
                        let progress = *count as f32 / total as f32;
                        state.set_completed_count(*count as i32);
                        state.set_progress(progress);

                        if *count >= total {
                            state.set_status_text(format!("完成！已压缩 {total} 个文件").into());
                        } else {
                            state.set_status_text(
                                format!("正在压缩... {}/{total}", *count).into(),
                            );
                        }
                    })
                    .ok();
                });
            });
        }
    });

    // ===== 清空 =====
    bridge.on_clear_all({
        let ui_handle = ui.as_weak();
        move || {
            tracing::info!("Clear all requested");
            let ui = ui_handle.unwrap();
            let state = ui.global::<AppState>();
            let empty_model: slint::VecModel<ImageItem> = slint::VecModel::default();
            state.set_images(slint::ModelRc::new(empty_model));
            state.set_progress(0.0);
            state.set_total_count(0);
            state.set_completed_count(0);
            state.set_status_text("就绪".into());
        }
    });

    // ===== 删除单个图片 =====
    bridge.on_remove_image({
        let ui_handle = ui.as_weak();
        move |id| {
            let ui = ui_handle.unwrap();
            let state = ui.global::<AppState>();
            let model = state.get_images();

            let vec_model = model
                .as_any()
                .downcast_ref::<slint::VecModel<ImageItem>>()
                .expect("images model should be VecModel");

            for i in 0..vec_model.row_count() {
                if let Some(item) = vec_model.row_data(i) {
                    if item.id == id {
                        vec_model.remove(i);
                        state.set_total_count(vec_model.row_count() as i32);
                        break;
                    }
                }
            }
        }
    });
}

/// 压缩单个文件（在工作线程中调用）
fn compress_single_file(
    input_path: &Path,
    gpu: &GpuAccelerator,
) -> anyhow::Result<(u64, PathBuf)> {
    // 探测格式
    let data = std::fs::read(input_path)?;
    let format = ImageFormat::from_magic_bytes(&data)
        .or_else(|| {
            input_path
                .extension()
                .and_then(|e| e.to_str())
                .and_then(ImageFormat::from_extension)
        })
        .ok_or_else(|| anyhow::anyhow!("Unable to detect image format"))?;

    // 获取编码参数
    let params = default_encode_params(format);

    // 获取实际输出格式
    let output_format = params.output_format().unwrap_or(format);

    // 构建预处理链（GPU 加速的缩放 + 元数据剥离）
    let mut preprocessors: Vec<Box<dyn engine::preprocess::Preprocessor>> = Vec::new();

    // 添加缩放预处理器（自动选择 GPU/CPU）
    // TODO: 从用户配置读取 max_width/max_height，当前仅在用户设置了缩放时才添加
    // preprocessors.push(gpu::create_resize_processor(gpu, max_width, max_height));

    // 添加元数据剥离
    preprocessors.push(Box::new(MetadataStripper::strip_all()));

    // 构建管线
    let decoder = Box::new(UniversalDecoder);
    let encoder = create_encoder(output_format, gpu)?;

    let pipeline = CompressionPipeline::new(decoder, preprocessors, encoder, params);

    // 执行压缩
    let result = pipeline.run(input_path)?;

    // 生成输出路径（同目录，添加 _compressed 后缀）
    let stem = input_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("output");
    let ext = output_format.extension();
    let output_path = input_path
        .parent()
        .unwrap_or(Path::new("."))
        .join(format!("{stem}_compressed.{ext}"));

    // 写入文件
    std::fs::write(&output_path, &result.data)?;

    Ok((result.compressed_size, output_path))
}

/// 根据格式创建编码器（GPU 可用时自动加速 JPEG）
fn create_encoder(
    format: ImageFormat,
    gpu_acc: &GpuAccelerator,
) -> anyhow::Result<Box<dyn engine::codec::Encoder>> {
    use engine::codec::{
        avif::AvifEncoder, gif::GifEncoder, png::OxipngEncoder, webp::WebpEncoder,
    };

    match format {
        ImageFormat::Jpeg => Ok(gpu::create_jpeg_encoder(gpu_acc)),
        ImageFormat::Png => Ok(Box::new(OxipngEncoder)),
        ImageFormat::WebP => Ok(Box::new(WebpEncoder)),
        ImageFormat::Avif => Ok(Box::new(AvifEncoder)),
        ImageFormat::Gif => Ok(Box::new(GifEncoder)),
        _ => anyhow::bail!("{format} encoding is not supported"),
    }
}
