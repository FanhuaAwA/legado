use crate::state::AppState;
use reader_core::{CommandError, SourceUpdateCheck};
use tauri::State;

type CommandResult<T> = Result<T, CommandError>;

fn map_err(err: reader_core::ReaderCoreError) -> CommandError {
    err.into_command_error()
}

/// 检测单个 JS 书源在其 `@updateUrl` 处是否有新版本。
#[tauri::command]
pub async fn booksource_check_update(
    state: State<'_, AppState>,
    file_name: String,
    source_dir: Option<String>,
) -> CommandResult<SourceUpdateCheck> {
    state
        .core
        .check_source_update(&file_name, source_dir.as_deref())
        .await
        .map_err(map_err)
}

/// 从 `@updateUrl` 拉取最新内容并覆盖本地文件（保留本地 `@enabled` 状态）。
#[tauri::command]
pub async fn booksource_apply_update(
    state: State<'_, AppState>,
    file_name: String,
    source_dir: Option<String>,
) -> CommandResult<()> {
    state
        .core
        .apply_source_update(&file_name, source_dir.as_deref())
        .await
        .map_err(map_err)
}
