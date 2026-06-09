mod commands;
mod state;

use reader_core::{ReaderCore, ReaderCoreOptions, SecureMode};
use state::AppState;
use std::sync::Arc;
#[cfg(target_os = "macos")]
use tauri::WindowEvent;
use tauri::{Emitter, Manager};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,reader_core=info,legado_tauri=info".into()),
        )
        .try_init()
        .ok();

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
            Ok(())
        })
        .invoke_handler(commands::handler())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
