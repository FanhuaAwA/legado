use crate::state::AppState;
use reader_core::CommandError;
use serde::Serialize;
use tauri::{Emitter, State};

#[tauri::command]
pub fn frontend_log(level: String, message: String) {
    match level.as_str() {
        "error" => tracing::error!(target: "frontend", "{message}"),
        "warning" => tracing::warn!(target: "frontend", "{message}"),
        "success" | "info" => tracing::info!(target: "frontend", "{message}"),
        _ => tracing::debug!(target: "frontend", level = %level, "{message}"),
    }
}

#[tauri::command]
pub fn get_platform() -> &'static str {
    #[cfg(target_os = "windows")]
    {
        "windows"
    }
    #[cfg(target_os = "macos")]
    {
        "macos"
    }
    #[cfg(target_os = "linux")]
    {
        "linux"
    }
    #[cfg(target_os = "android")]
    {
        "android"
    }
    #[cfg(target_os = "ios")]
    {
        "ios"
    }
    #[cfg(not(any(
        target_os = "windows",
        target_os = "macos",
        target_os = "linux",
        target_os = "android",
        target_os = "ios"
    )))]
    {
        "unknown"
    }
}

#[tauri::command]
pub async fn open_dir_in_explorer(path: String) -> Result<(), CommandError> {
    tauri_plugin_opener::open_path(path, None::<&str>).map_err(|err| CommandError {
        code: "IO_ERROR".to_string(),
        message: err.to_string(),
        detail: Some(format!("{err:?}")),
        retryable: false,
    })
}

#[tauri::command]
pub async fn script_dialog_result(
    app: tauri::AppHandle,
    id: String,
    value: serde_json::Value,
) -> Result<(), CommandError> {
    let _ = app.emit(
        "script:dialog:result",
        serde_json::json!({
            "id": id,
            "value": value
        }),
    );
    Ok(())
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioResolveRequest {
    pub url: String,
    pub referer: String,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioResolveRequestWrapper {
    pub request: AudioResolveRequest,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioResolveResponse {
    local_path: String,
}

#[tauri::command]
pub async fn audio_resolve_cache(
    state: State<'_, AppState>,
    request: AudioResolveRequestWrapper,
) -> Result<AudioResolveResponse, CommandError> {
    let local_path = state
        .core
        .resolve_audio_cache(&request.request.url, &request.request.referer)
        .await
        .map_err(|err| err.into_command_error())?;
    Ok(AudioResolveResponse { local_path })
}

#[tauri::command]
pub async fn script_repl_eval(
    state: State<'_, AppState>,
    code: String,
    context_file: Option<String>,
) -> Result<String, CommandError> {
    state
        .core
        .eval_repl(&code, context_file.as_deref())
        .map_err(|err| err.into_command_error())
}
