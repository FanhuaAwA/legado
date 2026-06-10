// pub 是为了 tests/ 集成测试与未来无头服务端复用（R-P2-008），bin 入口仍只用 run()
pub mod commands;
pub mod state;
pub mod ws_server;

use reader_core::{ReaderCore, ReaderCoreOptions, SecureMode};
use state::AppState;
use std::sync::Arc;
#[cfg(target_os = "macos")]
use tauri::WindowEvent;
use tauri::{Emitter, Manager};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::Layer;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Set up file-based logging with rotation
    let app_data = std::env::var("APPDATA")
        .or_else(|_| std::env::var("HOME").map(|h| format!("{}/.local/share", h)))
        .unwrap_or_else(|_| ".".to_string());
    let log_dir = std::path::PathBuf::from(&app_data)
        .join("com.legado.tauri")
        .join("reader")
        .join("logs");

    // Create the log directory
    if let Err(e) = std::fs::create_dir_all(&log_dir) {
        eprintln!("Failed to create log dir: {e}");
    }

    let file_appender = tracing_appender::rolling::daily(&log_dir, "app.log");
    let error_appender = tracing_appender::rolling::daily(&log_dir, "app.error.log");

    // Console layer (for development)
    let console_layer = tracing_subscriber::fmt::layer()
        .with_writer(std::io::stderr)
        .with_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,reader_core=info,legado_tauri=info".into()),
        );

    // File layer: all logs (info and above)
    let file_layer = tracing_subscriber::fmt::layer()
        .with_writer(file_appender)
        .with_ansi(false)
        .with_filter(tracing_subscriber::filter::LevelFilter::INFO);

    // Error file layer: warnings and errors
    let error_layer = tracing_subscriber::fmt::layer()
        .with_writer(error_appender)
        .with_ansi(false)
        .with_filter(tracing_subscriber::filter::LevelFilter::WARN);

    tracing_subscriber::registry()
        .with(console_layer)
        .with(file_layer)
        .with(error_layer)
        .init();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_deep_link::init())
        .on_window_event(|window, event| {
            #[cfg(not(target_os = "macos"))]
            let _ = (window, event);

            #[cfg(target_os = "macos")]
            {
                if window.label() == "main" {
                    if let WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        window.app_handle().exit(0);
                    }
                }
            }
        })
        .setup(|app| {
            let app_data_dir = app.path().app_data_dir().unwrap_or_else(|_| {
                app.path()
                    .home_dir()
                    .unwrap_or_else(|_| std::env::temp_dir())
            });
            let app_handle = app.handle().clone();
            let core = tauri::async_runtime::block_on(async move {
                ReaderCore::new(ReaderCoreOptions {
                    app_data_dir,
                    request_timeout_secs: 35,
                    user_agent: None,
                    secure_mode: SecureMode::Normal,
                })
                .await
            })?;
            app.manage(AppState {
                core: Arc::new(core),
                tasks: state::TaskRegistry::default(),
            });
            #[cfg(target_os = "macos")]
            if let Some(window) = app.get_webview_window("main") {
                window.set_decorations(true)?;
            }
            let _ = app_handle.emit(
                "rust:log",
                serde_json::json!({"message": "reader-core initialized"}),
            );
            // R-P2-008 阶段 2 试点：应用内 WS 命令服务端（仅 127.0.0.1:7688，
            // 协议与白名单见 docs/frontend-backend-separation.md）
            ws_server::start(app_handle.clone());
            Ok(())
        })
        .invoke_handler(commands::handler())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
