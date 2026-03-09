mod app;
mod config;
mod engine;
mod gpu;
mod i18n;
mod worker;

use app::App;
use config::preset::CompressionPreset;
use config::storage::ConfigStorage;
use config::{AppConfig, OutputDir};
use engine::codec::universal::UniversalDecoder;
use engine::params::{EncodeParams, ImageFormat};
use engine::pipeline::CompressionPipeline;
use engine::preprocess::metadata::MetadataStripper;
use gpu::context::GpuAccelerator;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use slint::Model;

slint::include_modules!();

/// 从 UI 状态收集的压缩设置
#[derive(Debug, Clone)]
struct CompressSettings {
    // 按格式参数
    jpeg_quality: u8,
    jpeg_progressive: bool,
    png_lossy: bool,
    png_optimization: u8,
    webp_quality: u8,
    webp_lossless: bool,
    avif_quality: u8,
    avif_speed: u8,
    gif_quality: u8,
    gif_fast: bool,
    // 输出
    output_format_index: i32, // 0=原格式, 1=JPEG, 2=PNG, 3=WebP, 4=AVIF, 5=GIF
    suffix: String,
    same_dir: bool,
    custom_dir: String,
    overwrite: bool,
    // 处理
    strip_metadata: bool,
    resize_enabled: bool,
    max_width: u32,
    max_height: u32,
}

impl CompressSettings {
    /// 从 AppState 读取当前设置（必须在 UI 线程调用）
    fn from_ui(state: &AppState) -> Self {
        Self {
            jpeg_quality: (state.get_jpeg_quality() as u8).clamp(1, 100),
            jpeg_progressive: state.get_jpeg_progressive(),
            png_lossy: state.get_png_lossy(),
            png_optimization: (state.get_png_optimization() as u8).clamp(1, 6),
            webp_quality: (state.get_webp_quality() as u8).clamp(1, 100),
            webp_lossless: state.get_webp_lossless(),
            avif_quality: (state.get_avif_quality() as u8).clamp(1, 100),
            avif_speed: (state.get_avif_speed() as u8).clamp(1, 10),
            gif_quality: (state.get_gif_quality() as u8).clamp(1, 100),
            gif_fast: state.get_gif_fast(),
            output_format_index: state.get_output_format_index(),
            suffix: state.get_output_suffix().to_string(),
            same_dir: state.get_output_same_dir(),
            custom_dir: state.get_output_custom_dir().to_string(),
            overwrite: state.get_overwrite(),
            strip_metadata: state.get_strip_metadata(),
            resize_enabled: state.get_resize_enabled(),
            max_width: state.get_max_width().max(0) as u32,
            max_height: state.get_max_height().max(0) as u32,
        }
    }

    /// 获取用户指定的输出格式（None 表示保持原格式）
    fn target_format(&self) -> Option<ImageFormat> {
        match self.output_format_index {
            1 => Some(ImageFormat::Jpeg),
            2 => Some(ImageFormat::Png),
            3 => Some(ImageFormat::WebP),
            4 => Some(ImageFormat::Avif),
            5 => Some(ImageFormat::Gif),
            _ => None, // 0 或其他 = 保持原格式
        }
    }

    /// 根据源格式返回编码参数
    fn encode_params(&self, format: ImageFormat) -> EncodeParams {
        match format {
            ImageFormat::Jpeg => EncodeParams::Jpeg {
                quality: self.jpeg_quality,
                progressive: self.jpeg_progressive,
            },
            ImageFormat::Png => EncodeParams::Png {
                lossy: self.png_lossy,
                optimization_level: self.png_optimization,
            },
            ImageFormat::WebP => EncodeParams::WebP {
                quality: self.webp_quality,
                lossless: self.webp_lossless,
            },
            ImageFormat::Avif => EncodeParams::Avif {
                quality: self.avif_quality,
                speed: self.avif_speed,
            },
            ImageFormat::Gif => EncodeParams::Gif {
                quality: self.gif_quality,
                fast: self.gif_fast,
            },
            // JXL/SVG 暂不支持，fallback 到 PNG
            _ => EncodeParams::Png {
                lossy: false,
                optimization_level: 2,
            },
        }
    }

    /// 生成输出路径
    fn output_path(&self, input_path: &Path, output_format: ImageFormat) -> PathBuf {
        let stem = input_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("output");
        let ext = output_format.extension();

        let dir = if self.same_dir || self.custom_dir.is_empty() {
            input_path
                .parent()
                .unwrap_or(Path::new("."))
                .to_path_buf()
        } else {
            PathBuf::from(&self.custom_dir)
        };

        if self.overwrite {
            // 覆盖模式：直接使用原文件名（扩展名可能变化）
            dir.join(format!("{stem}.{ext}"))
        } else {
            dir.join(format!("{stem}{}.{ext}", self.suffix))
        }
    }
}

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
        "jpg", "jpeg", "png", "webp", "avif", "jxl", "gif", "svg", "svgz", "bmp", "tiff", "tif",
        "ico",
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

        let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);

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

/// 将预设参数加载到 AppState
fn apply_preset_to_state(state: &AppState, preset: &CompressionPreset) {
    state.set_jpeg_quality(preset.jpeg.quality as i32);
    state.set_jpeg_progressive(preset.jpeg.progressive);
    state.set_png_lossy(preset.png.lossy);
    state.set_png_optimization(preset.png.optimization_level as i32);
    state.set_webp_quality(preset.webp.quality as i32);
    state.set_webp_lossless(preset.webp.lossless);
    state.set_avif_quality(preset.avif.quality as i32);
    state.set_avif_speed(preset.avif.speed as i32);
    state.set_gif_quality(preset.gif.quality as i32);
    state.set_gif_fast(preset.gif.fast);
    state.set_strip_metadata(preset.strip_metadata);

    if let Some(ref resize) = preset.resize {
        state.set_resize_enabled(true);
        state.set_max_width(resize.max_width.unwrap_or(0) as i32);
        state.set_max_height(resize.max_height.unwrap_or(0) as i32);
    } else {
        state.set_resize_enabled(false);
        state.set_max_width(0);
        state.set_max_height(0);
    }
}

/// 获取所有预设（内置 + 用户自定义）
fn all_presets(config: &AppConfig) -> Vec<CompressionPreset> {
    let mut presets = CompressionPreset::builtin_presets();
    presets.extend(config.presets.clone());
    presets
}

/// 填充预设列表到 UI
fn populate_presets(state: &AppState, presets: &[CompressionPreset]) {
    // 填充 PresetItem 列表
    let preset_model = slint::VecModel::default();
    for p in presets {
        preset_model.push(PresetItem {
            name: p.name.clone().into(),
            description: p.description.clone().into(),
            builtin: p.builtin,
        });
    }
    state.set_presets(slint::ModelRc::new(preset_model));

    // 填充 preset-names（用于 ComboBox）
    let names_model = slint::VecModel::default();
    for p in presets {
        names_model.push(slint::SharedString::from(&p.description));
    }
    state.set_preset_names(slint::ModelRc::new(names_model));
}

fn setup_callbacks(ui: &AppWindow, app: App) {
    let bridge = ui.global::<AppBridge>();

    // 共享状态
    let gpu = Arc::new(app.gpu);
    let config = Arc::new(Mutex::new(app.config));

    // 初始化图片列表 model
    {
        let state = ui.global::<AppState>();
        let model: slint::VecModel<ImageItem> = slint::VecModel::default();
        state.set_images(slint::ModelRc::new(model));

        // 设置 GPU 状态
        state.set_gpu_available(gpu.is_available());
        #[cfg(feature = "gpu")]
        {
            state.set_gpu_name(gpu.name().into());
        }

        // 加载配置到 UI
        let cfg = config.lock().unwrap();
        state.set_output_suffix(cfg.suffix.clone().into());
        state.set_overwrite(cfg.overwrite);
        match &cfg.output_dir {
            OutputDir::SameAsInput => {
                state.set_output_same_dir(true);
                state.set_output_custom_dir(slint::SharedString::default());
            }
            OutputDir::Custom(dir) => {
                state.set_output_same_dir(false);
                state.set_output_custom_dir(dir.clone().into());
            }
        }

        // 填充预设列表
        let presets = all_presets(&cfg);
        populate_presets(&state, &presets);

        // 选中默认预设并加载参数
        let default_name = &cfg.default_preset;
        let default_idx = presets
            .iter()
            .position(|p| p.name == *default_name)
            .unwrap_or(0);
        state.set_current_preset_index(default_idx as i32);
        state.set_current_preset(presets[default_idx].name.clone().into());
        apply_preset_to_state(&state, &presets[default_idx]);
    }

    // ===== 添加文件 =====
    bridge.on_add_files({
        let ui_handle = ui.as_weak();
        move || {
            tracing::info!("Add files requested");
            let ui = ui_handle.unwrap();

            let dialog = rfd::FileDialog::new()
                .set_title("选择图片文件")
                .add_filter(
                    "图片文件",
                    &[
                        "jpg", "jpeg", "png", "webp", "avif", "gif", "bmp", "tiff", "ico",
                    ],
                )
                .add_filter("所有文件", &["*"]);

            if let Some(paths) = dialog.pick_files() {
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

            // 从 UI 读取压缩设置
            let settings = CompressSettings::from_ui(&state);

            // 确保自定义输出目录存在
            if !settings.same_dir && !settings.custom_dir.is_empty() {
                let dir = Path::new(&settings.custom_dir);
                if !dir.exists() {
                    if let Err(e) = std::fs::create_dir_all(dir) {
                        state.set_status_text(format!("无法创建输出目录: {e}").into());
                        return;
                    }
                }
            }

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
            let settings = Arc::new(settings);

            std::thread::spawn(move || {
                use rayon::prelude::*;

                tasks.par_iter().for_each(|(id, input_path)| {
                    if cancel.load(Ordering::Relaxed) {
                        return;
                    }

                    let task_id = *id;
                    let result = compress_single_file(input_path, &gpu, &settings);

                    let ui_handle = ui_handle.clone();
                    let completed_count = Arc::clone(&completed_count);

                    // 回到 UI 线程更新状态
                    slint::invoke_from_event_loop(move || {
                        let ui = ui_handle.unwrap();
                        let state = ui.global::<AppState>();
                        let model = state.get_images();

                        for i in 0..model.row_count() {
                            if let Some(mut item) = model.row_data(i) {
                                if item.id == task_id {
                                    match &result {
                                        Ok((compressed_size, output_path)) => {
                                            item.status = "completed".into();
                                            item.compressed_size =
                                                format_size(*compressed_size).into();

                                            let original = std::fs::metadata(PathBuf::from(
                                                item.filepath.as_str(),
                                            ))
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

    // ===== 选择预设 =====
    bridge.on_select_preset({
        let ui_handle = ui.as_weak();
        let config = Arc::clone(&config);
        move |preset_desc| {
            let ui = ui_handle.unwrap();
            let state = ui.global::<AppState>();
            let cfg = config.lock().unwrap();
            let presets = all_presets(&cfg);

            // 通过描述匹配预设（ComboBox selected 返回显示文本即描述）
            if let Some(preset) = presets.iter().find(|p| p.description == preset_desc.as_str()) {
                tracing::info!("Selected preset: {}", preset.name);
                state.set_current_preset(preset.name.clone().into());
                apply_preset_to_state(&state, preset);
            }
        }
    });

    // ===== 保存自定义预设 =====
    bridge.on_save_custom_preset({
        let ui_handle = ui.as_weak();
        let config = Arc::clone(&config);
        move |name| {
            let ui = ui_handle.unwrap();
            let state = ui.global::<AppState>();
            let settings = CompressSettings::from_ui(&state);

            let preset = CompressionPreset {
                name: name.to_string(),
                description: name.to_string(),
                jpeg: config::preset::JpegPreset {
                    quality: settings.jpeg_quality,
                    progressive: settings.jpeg_progressive,
                },
                png: config::preset::PngPreset {
                    lossy: settings.png_lossy,
                    optimization_level: settings.png_optimization,
                },
                webp: config::preset::WebpPreset {
                    quality: settings.webp_quality,
                    lossless: settings.webp_lossless,
                },
                avif: config::preset::AvifPreset {
                    quality: settings.avif_quality,
                    speed: settings.avif_speed,
                },
                jxl: config::preset::JxlPreset {
                    quality: 75,
                    effort: 7,
                },
                gif: config::preset::GifPreset {
                    quality: settings.gif_quality,
                    fast: settings.gif_fast,
                },
                resize: if settings.resize_enabled {
                    Some(config::preset::ResizePreset {
                        max_width: if settings.max_width > 0 {
                            Some(settings.max_width)
                        } else {
                            None
                        },
                        max_height: if settings.max_height > 0 {
                            Some(settings.max_height)
                        } else {
                            None
                        },
                        maintain_aspect_ratio: true,
                    })
                } else {
                    None
                },
                strip_metadata: settings.strip_metadata,
                builtin: false,
            };

            // 保存到配置
            let mut cfg = config.lock().unwrap();
            // 移除同名旧预设
            cfg.presets.retain(|p| p.name != name.as_str());
            cfg.presets.push(preset);

            if let Err(e) = ConfigStorage::save(&cfg) {
                tracing::error!("Failed to save config: {e}");
            }

            // 刷新 UI 预设列表
            let presets = all_presets(&cfg);
            populate_presets(&state, &presets);

            tracing::info!("Saved custom preset: {name}");
        }
    });

    // ===== 删除预设 =====
    bridge.on_delete_preset({
        let ui_handle = ui.as_weak();
        let config = Arc::clone(&config);
        move |name| {
            let mut cfg = config.lock().unwrap();

            // 不能删除内置预设
            let is_builtin = CompressionPreset::builtin_presets()
                .iter()
                .any(|p| p.name == name.as_str());

            if is_builtin {
                tracing::warn!("Cannot delete builtin preset: {name}");
                return;
            }

            cfg.presets.retain(|p| p.name != name.as_str());

            if let Err(e) = ConfigStorage::save(&cfg) {
                tracing::error!("Failed to save config: {e}");
            }

            // 刷新 UI
            let ui = ui_handle.unwrap();
            let state = ui.global::<AppState>();
            let presets = all_presets(&cfg);
            populate_presets(&state, &presets);

            tracing::info!("Deleted preset: {name}");
        }
    });

    // ===== 浏览输出目录 =====
    bridge.on_browse_output_dir({
        let ui_handle = ui.as_weak();
        let config = Arc::clone(&config);
        move || {
            let dialog = rfd::FileDialog::new().set_title("选择输出目录");
            if let Some(folder) = dialog.pick_folder() {
                let dir_str = folder.to_string_lossy().to_string();
                let ui = ui_handle.unwrap();
                let state = ui.global::<AppState>();
                state.set_output_custom_dir(dir_str.clone().into());
                state.set_output_same_dir(false);

                // 保存到配置
                let mut cfg = config.lock().unwrap();
                cfg.output_dir = OutputDir::Custom(dir_str);
                if let Err(e) = ConfigStorage::save(&cfg) {
                    tracing::error!("Failed to save config: {e}");
                }
            }
        }
    });

    // ===== 切换语言 =====
    bridge.on_change_language({
        let config = Arc::clone(&config);
        move |lang| {
            tracing::info!("Language changed to: {lang}");
            i18n::set_language(lang.as_str());

            let mut cfg = config.lock().unwrap();
            cfg.language = lang.to_string();
            if let Err(e) = ConfigStorage::save(&cfg) {
                tracing::error!("Failed to save config: {e}");
            }
        }
    });

    // ===== 切换主题 =====
    bridge.on_change_theme({
        let config = Arc::clone(&config);
        move |theme| {
            tracing::info!("Theme changed to: {theme}");

            let mut cfg = config.lock().unwrap();
            cfg.theme = match theme.as_str() {
                "light" => config::Theme::Light,
                "dark" => config::Theme::Dark,
                _ => config::Theme::System,
            };
            if let Err(e) = ConfigStorage::save(&cfg) {
                tracing::error!("Failed to save config: {e}");
            }
        }
    });

    // ===== 压缩单个 =====
    bridge.on_compress_single({
        let ui_handle = ui.as_weak();
        let gpu = Arc::clone(&gpu);
        move |task_id| {
            tracing::info!("Compress single: {task_id}");
            let ui = ui_handle.unwrap();
            let state = ui.global::<AppState>();

            let settings = CompressSettings::from_ui(&state);

            // 找到对应文件
            let model = state.get_images();
            let mut target: Option<PathBuf> = None;
            for i in 0..model.row_count() {
                if let Some(mut item) = model.row_data(i) {
                    if item.id == task_id {
                        target = Some(PathBuf::from(item.filepath.as_str()));
                        item.status = "processing".into();
                        model.set_row_data(i, item);
                        break;
                    }
                }
            }

            let Some(input_path) = target else { return };

            let ui_handle = ui.as_weak();
            let gpu = Arc::clone(&gpu);
            let settings = Arc::new(settings);

            std::thread::spawn(move || {
                let result = compress_single_file(&input_path, &gpu, &settings);
                slint::invoke_from_event_loop(move || {
                    let ui = ui_handle.unwrap();
                    let state = ui.global::<AppState>();
                    let model = state.get_images();
                    for i in 0..model.row_count() {
                        if let Some(mut item) = model.row_data(i) {
                            if item.id == task_id {
                                match &result {
                                    Ok((compressed_size, output_path)) => {
                                        item.status = "completed".into();
                                        item.compressed_size =
                                            format_size(*compressed_size).into();
                                        let original = std::fs::metadata(PathBuf::from(
                                            item.filepath.as_str(),
                                        ))
                                        .map(|m| m.len())
                                        .unwrap_or(0);
                                        if original > 0 {
                                            let ratio =
                                                1.0 - (*compressed_size as f64 / original as f64);
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
                                    }
                                }
                                model.set_row_data(i, item);
                                break;
                            }
                        }
                    }
                })
                .ok();
            });
        }
    });

    // ===== 请求预览（暂用空实现）=====
    bridge.on_request_preview(|_id| {
        tracing::info!("Request preview: {_id} (not yet implemented)");
    });
}

/// 压缩单个文件（在工作线程中调用）
fn compress_single_file(
    input_path: &Path,
    gpu: &GpuAccelerator,
    settings: &CompressSettings,
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

    // 确定输出格式（用户指定 > 原格式）
    let mut output_format = settings.target_format().unwrap_or(format);

    // JXL/SVG 无法编码，自动降级为 PNG
    if matches!(output_format, ImageFormat::Jxl | ImageFormat::Svg) {
        tracing::warn!(
            "{output_format} encoding not supported, falling back to PNG"
        );
        output_format = ImageFormat::Png;
    }

    // 从设置获取编码参数
    let params = settings.encode_params(output_format);

    // 构建预处理链
    let mut preprocessors: Vec<Box<dyn engine::preprocess::Preprocessor>> = Vec::new();

    // 缩放预处理器
    if settings.resize_enabled {
        let max_w = if settings.max_width > 0 {
            Some(settings.max_width)
        } else {
            None
        };
        let max_h = if settings.max_height > 0 {
            Some(settings.max_height)
        } else {
            None
        };
        if max_w.is_some() || max_h.is_some() {
            preprocessors.push(gpu::create_resize_processor(gpu, max_w, max_h));
        }
    }

    // 元数据剥离
    if settings.strip_metadata {
        preprocessors.push(Box::new(MetadataStripper::strip_all()));
    }

    // 构建管线
    let decoder = Box::new(UniversalDecoder);
    let encoder = create_encoder(output_format, gpu)?;
    let pipeline = CompressionPipeline::new(decoder, preprocessors, encoder, params);

    // 执行压缩
    let result = pipeline.run(input_path)?;

    // 生成输出路径
    let output_path = settings.output_path(input_path, output_format);

    // 确保输出目录存在
    if let Some(parent) = output_path.parent() {
        if !parent.exists() {
            std::fs::create_dir_all(parent)?;
        }
    }

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
