mod app;
mod config;
mod engine;
mod gpu;
mod i18n;
mod worker;

use app::App;

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

fn setup_callbacks(ui: &AppWindow, _app: App) {
    // 获取 bridge 回调注册
    let bridge = ui.global::<AppBridge>();

    // 添加文件
    bridge.on_add_files({
        let ui_handle = ui.as_weak();
        move || {
            tracing::info!("Add files requested");
            let _ui = ui_handle.unwrap();
            // TODO: 打开文件选择对话框
            // TODO: 将选中的文件添加到图片列表
        }
    });

    // 添加文件夹
    bridge.on_add_folder({
        let ui_handle = ui.as_weak();
        move || {
            tracing::info!("Add folder requested");
            let _ui = ui_handle.unwrap();
            // TODO: 打开文件夹选择对话框
            // TODO: 递归扫描并添加图片
        }
    });

    // 压缩全部
    bridge.on_compress_all({
        let ui_handle = ui.as_weak();
        move || {
            tracing::info!("Compress all requested");
            let _ui = ui_handle.unwrap();
            // TODO: 启动批量压缩任务
        }
    });

    // 取消
    bridge.on_cancel({
        move || {
            tracing::info!("Cancel requested");
            // TODO: 取消当前压缩任务
        }
    });

    // 清空
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
}
