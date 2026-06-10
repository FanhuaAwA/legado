use crate::state::AppState;
use reader_core::{CommandError, FrontendStorageEntry, FrontendStorageNamespaceSummary};
use serde_json::Value;
use tauri::{Emitter, State};

type CommandResult<T> = Result<T, CommandError>;

fn map_err(err: reader_core::ReaderCoreError) -> CommandError {
    err.into_command_error()
}

#[tauri::command]
pub async fn config_read(
    state: State<'_, AppState>,
    scope: String,
    key: String,
) -> CommandResult<String> {
    state.core.config_read(&scope, &key).await.map_err(map_err)
}

#[tauri::command]
pub async fn config_write(
    state: State<'_, AppState>,
    scope: String,
    key: String,
    value: String,
) -> CommandResult<()> {
    state
        .core
        .config_write(&scope, &key, &value)
        .await
        .map_err(map_err)
}

#[tauri::command]
pub async fn config_read_json(
    state: State<'_, AppState>,
    scope: String,
    key: String,
) -> CommandResult<Option<Value>> {
    state
        .core
        .config_read_json(&scope, &key)
        .await
        .map_err(map_err)
}

#[tauri::command]
pub async fn config_write_json(
    state: State<'_, AppState>,
    scope: String,
    key: String,
    value: Value,
) -> CommandResult<()> {
    state
        .core
        .config_write_json(&scope, &key, &value)
        .await
        .map_err(map_err)
}

#[tauri::command]
pub async fn config_delete_key(
    state: State<'_, AppState>,
    scope: String,
    key: String,
) -> CommandResult<()> {
    state
        .core
        .config_delete_key(&scope, &key)
        .await
        .map_err(map_err)
}

#[tauri::command]
pub async fn config_read_all(state: State<'_, AppState>, scope: String) -> CommandResult<String> {
    state.core.config_read_all(&scope).await.map_err(map_err)
}

#[tauri::command]
pub async fn config_clear(state: State<'_, AppState>, scope: String) -> CommandResult<()> {
    state.core.config_clear(&scope).await.map_err(map_err)
}

#[tauri::command]
pub async fn config_read_bytes(
    state: State<'_, AppState>,
    scope: String,
    key: String,
) -> CommandResult<Vec<u8>> {
    let value = state
        .core
        .config_read_json(&scope, &key)
        .await
        .map_err(map_err)?;
    Ok(value
        .and_then(|value| serde_json::from_value::<Vec<u8>>(value).ok())
        .unwrap_or_default())
}

#[tauri::command]
pub async fn config_write_bytes(
    state: State<'_, AppState>,
    scope: String,
    key: String,
    value: Vec<u8>,
) -> CommandResult<()> {
    state
        .core
        .config_write_json(
            &scope,
            &key,
            &serde_json::to_value(value).unwrap_or(Value::Null),
        )
        .await
        .map_err(map_err)
}

#[tauri::command]
pub async fn config_list_scopes(state: State<'_, AppState>) -> CommandResult<Vec<String>> {
    state.core.config_list_scopes().await.map_err(map_err)
}

#[tauri::command]
pub async fn config_dump_scope(state: State<'_, AppState>, scope: String) -> CommandResult<String> {
    state.core.config_read_all(&scope).await.map_err(map_err)
}

#[tauri::command]
pub async fn frontend_storage_list(
    state: State<'_, AppState>,
    namespace: String,
) -> CommandResult<Vec<FrontendStorageEntry>> {
    state
        .core
        .frontend_storage_list(&namespace)
        .await
        .map_err(map_err)
}

#[tauri::command]
pub async fn frontend_storage_set(
    state: State<'_, AppState>,
    namespace: String,
    key: String,
    value: String,
) -> CommandResult<()> {
    state
        .core
        .frontend_storage_set(&namespace, &key, &value)
        .await
        .map_err(map_err)
}

#[tauri::command]
pub async fn frontend_storage_remove(
    state: State<'_, AppState>,
    namespace: String,
    key: String,
) -> CommandResult<()> {
    state
        .core
        .frontend_storage_remove(&namespace, &key)
        .await
        .map_err(map_err)
}

#[tauri::command]
pub async fn frontend_storage_list_namespaces(
    state: State<'_, AppState>,
) -> CommandResult<Vec<FrontendStorageNamespaceSummary>> {
    state
        .core
        .frontend_storage_list_namespaces()
        .await
        .map_err(map_err)
}

#[tauri::command]
pub async fn app_config_get_all(state: State<'_, AppState>) -> CommandResult<Value> {
    state.core.app_config_get_all().await.map_err(map_err)
}

// app_config_set / app_config_reset 需要 AppHandle 发事件，业务体收口为泛型 _impl，
// Tauri IPC 与 WS 路由（commands/router.rs）共用同一实现（R-P2-008 阶段 1 收口模式）。
pub async fn app_config_set_impl<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    state: &AppState,
    key: String,
    value: Value,
) -> CommandResult<()> {
    state
        .core
        .app_config_set(&key, &value)
        .await
        .map_err(map_err)?;
    let _ = app.emit("app_config:changed", serde_json::json!({ "key": key }));
    Ok(())
}

pub async fn app_config_reset_impl<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    state: &AppState,
    key: String,
) -> CommandResult<()> {
    state.core.app_config_reset(&key).await.map_err(map_err)?;
    let _ = app.emit("app_config:changed", serde_json::json!({ "key": key }));
    Ok(())
}

#[tauri::command]
pub async fn app_config_set(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    key: String,
    value: Value,
) -> CommandResult<()> {
    app_config_set_impl(&app, state.inner(), key, value).await
}

#[tauri::command]
pub async fn app_config_reset(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    key: String,
) -> CommandResult<()> {
    app_config_reset_impl(&app, state.inner(), key).await
}

#[tauri::command]
pub async fn storage_debug_dump(state: State<'_, AppState>) -> CommandResult<Value> {
    state.core.debug_dump().await.map_err(map_err)
}
