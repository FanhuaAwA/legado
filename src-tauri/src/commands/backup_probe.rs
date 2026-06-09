use crate::state::AppState;
use reader_core::CommandError;
use tauri::State;

type CommandResult<T> = Result<T, CommandError>;

fn map_err(err: reader_core::ReaderCoreError) -> CommandError {
    err.into_command_error()
}
fn unsupported(f: &str) -> CommandError {
    CommandError { code: "UNSUPPORTED".into(), message: format!("{f} 功能尚未实现"), detail: None, retryable: false }
}

#[tauri::command]
pub async fn backup_create(state: State<'_, AppState>) -> CommandResult<String> {
    state.core.create_backup().await.map_err(map_err)
}

#[tauri::command] pub async fn backup_create_data() -> CommandResult<()> { Err(unsupported("备份数据创建")) }
#[tauri::command] pub async fn backup_inspect() -> CommandResult<()> { Err(unsupported("备份检查")) }
#[tauri::command] pub async fn backup_peek() -> CommandResult<()> { Err(unsupported("备份预览")) }
#[tauri::command] pub async fn backup_peek_data() -> CommandResult<()> { Err(unsupported("备份数据预览")) }

#[tauri::command]
pub async fn backup_restore(state: State<'_, AppState>, path: String) -> CommandResult<i32> {
    state.core.restore_backup(&path).await.map_err(map_err)
}

#[tauri::command] pub async fn backup_restore_data() -> CommandResult<()> { Err(unsupported("备份数据恢复")) }

#[tauri::command] pub async fn browser_probe_create() -> CommandResult<()> { Err(unsupported("浏览器探测")) }
#[tauri::command] pub async fn browser_probe_close() -> CommandResult<()> { Err(unsupported("浏览器探测")) }
#[tauri::command] pub async fn browser_probe_close_all() -> CommandResult<()> { Err(unsupported("浏览器探测")) }
#[tauri::command] pub async fn browser_probe_hide() -> CommandResult<()> { Err(unsupported("浏览器探测")) }
#[tauri::command] pub async fn browser_probe_show() -> CommandResult<()> { Err(unsupported("浏览器探测")) }
#[tauri::command] pub async fn browser_probe_navigate() -> CommandResult<()> { Err(unsupported("浏览器探测")) }
#[tauri::command] pub async fn browser_probe_eval() -> CommandResult<()> { Err(unsupported("浏览器探测")) }
#[tauri::command] pub async fn browser_probe_run() -> CommandResult<()> { Err(unsupported("浏览器探测")) }
#[tauri::command] pub async fn browser_probe_get_cookies() -> CommandResult<()> { Err(unsupported("浏览器探测")) }
#[tauri::command] pub async fn browser_probe_set_cookie() -> CommandResult<()> { Err(unsupported("浏览器探测")) }
#[tauri::command] pub async fn browser_probe_clear_data() -> CommandResult<()> { Err(unsupported("浏览器探测")) }
#[tauri::command] pub async fn browser_probe_set_user_agent() -> CommandResult<()> { Err(unsupported("浏览器探测")) }
