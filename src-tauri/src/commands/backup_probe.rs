use crate::state::AppState;
use reader_core::{
    BackupCreateDataRequest, BackupCreateRequest, BackupPeekDataRequest, BackupPeekRequest,
    BackupRestoreDataRequest, BackupRestoreRequest, CommandError,
};
use serde::Serialize;
use tauri::State;

type CommandResult<T> = Result<T, CommandError>;

fn unsupported(feature: &str) -> CommandError {
    CommandError {
        code: "UNSUPPORTED".to_string(),
        message: format!("{feature} 功能尚未实现"),
        detail: None,
        retryable: false,
    }
}

fn io_error(err: impl std::fmt::Display) -> CommandError {
    CommandError {
        code: "IO_ERROR".to_string(),
        message: err.to_string(),
        detail: None,
        retryable: false,
    }
}

fn to_json_value<T: Serialize>(value: T) -> CommandResult<serde_json::Value> {
    serde_json::to_value(value).map_err(io_error)
}

#[tauri::command]
pub async fn backup_inspect(state: State<'_, AppState>) -> CommandResult<serde_json::Value> {
    state
        .core
        .backup_inspect()
        .await
        .map_err(|err| err.into_command_error())
        .and_then(to_json_value)
}

#[tauri::command]
pub async fn backup_create(
    state: State<'_, AppState>,
    request: BackupCreateRequest,
) -> CommandResult<serde_json::Value> {
    let (raw, categories) = state
        .core
        .backup_create_json(&request.categories)
        .await
        .map_err(|err| err.into_command_error())?;
    std::fs::write(&request.output_path, &raw).map_err(io_error)?;
    Ok(serde_json::json!({
        "outputPath": request.output_path,
        "byteSize": raw.len() as i64,
        "categories": categories,
    }))
}

#[tauri::command]
pub async fn backup_create_data(
    state: State<'_, AppState>,
    request: BackupCreateDataRequest,
) -> CommandResult<serde_json::Value> {
    state
        .core
        .backup_create_data(&request.default_name, &request.categories)
        .await
        .map_err(|err| err.into_command_error())
        .and_then(to_json_value)
}

#[tauri::command]
pub async fn backup_peek(request: BackupPeekRequest) -> CommandResult<serde_json::Value> {
    let raw = std::fs::read_to_string(&request.json_path).map_err(io_error)?;
    reader_core::ReaderCore::backup_peek_json(&raw)
        .map_err(|err| err.into_command_error())
        .and_then(to_json_value)
}

#[tauri::command]
pub async fn backup_peek_data(request: BackupPeekDataRequest) -> CommandResult<serde_json::Value> {
    reader_core::ReaderCore::backup_peek_data(&request.base64)
        .map_err(|err| err.into_command_error())
        .and_then(to_json_value)
}

#[tauri::command]
pub async fn backup_restore(
    state: State<'_, AppState>,
    request: BackupRestoreRequest,
) -> CommandResult<serde_json::Value> {
    let raw = std::fs::read_to_string(&request.json_path).map_err(io_error)?;
    state
        .core
        .backup_restore_json(&raw, &request.categories)
        .await
        .map_err(|err| err.into_command_error())
        .and_then(to_json_value)
}

#[tauri::command]
pub async fn backup_restore_data(
    state: State<'_, AppState>,
    request: BackupRestoreDataRequest,
) -> CommandResult<serde_json::Value> {
    state
        .core
        .backup_restore_data(&request.base64, &request.categories)
        .await
        .map_err(|err| err.into_command_error())
        .and_then(to_json_value)
}

#[tauri::command]
pub async fn browser_probe_create() -> CommandResult<()> {
    Err(unsupported("浏览器探测"))
}

#[tauri::command]
pub async fn browser_probe_close() -> CommandResult<()> {
    Err(unsupported("浏览器探测"))
}

#[tauri::command]
pub async fn browser_probe_close_all() -> CommandResult<()> {
    Err(unsupported("浏览器探测"))
}

#[tauri::command]
pub async fn browser_probe_hide() -> CommandResult<()> {
    Err(unsupported("浏览器探测"))
}

#[tauri::command]
pub async fn browser_probe_show() -> CommandResult<()> {
    Err(unsupported("浏览器探测"))
}

#[tauri::command]
pub async fn browser_probe_navigate() -> CommandResult<()> {
    Err(unsupported("浏览器探测"))
}

#[tauri::command]
pub async fn browser_probe_eval() -> CommandResult<()> {
    Err(unsupported("浏览器探测"))
}

#[tauri::command]
pub async fn browser_probe_run() -> CommandResult<()> {
    Err(unsupported("浏览器探测"))
}

#[tauri::command]
pub async fn browser_probe_get_cookies() -> CommandResult<()> {
    Err(unsupported("浏览器探测"))
}

#[tauri::command]
pub async fn browser_probe_set_cookie() -> CommandResult<()> {
    Err(unsupported("浏览器探测"))
}

#[tauri::command]
pub async fn browser_probe_clear_data() -> CommandResult<()> {
    Err(unsupported("浏览器探测"))
}

#[tauri::command]
pub async fn browser_probe_set_user_agent() -> CommandResult<()> {
    Err(unsupported("浏览器探测"))
}
