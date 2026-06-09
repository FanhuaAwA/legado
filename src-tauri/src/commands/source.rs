use crate::state::AppState;
use reader_core::{
    BookDetail, BookItem, BookSourceMeta, ChapterItem, CommandError, LegacyJsonImportResult,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::{Emitter, State};
#[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
use tauri_plugin_dialog::DialogExt;

type CommandResult<T> = Result<T, CommandError>;

fn map_err(err: reader_core::ReaderCoreError) -> CommandError {
    err.into_command_error()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteItem {
    pub file_name: String,
    pub source_dir: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteError {
    pub file_name: String,
    pub source_dir: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteBatchResult {
    pub deleted: Vec<DeleteItem>,
    pub errors: Vec<DeleteError>,
}

#[tauri::command]
pub async fn booksource_get_dir(state: State<'_, AppState>) -> CommandResult<String> {
    Ok(state.core.js_source_dir().to_string_lossy().to_string())
}

#[tauri::command]
pub async fn booksource_get_dirs(state: State<'_, AppState>) -> CommandResult<Vec<String>> {
    state.core.source_dirs().await.map_err(map_err)
}

#[tauri::command]
pub async fn booksource_add_dir(state: State<'_, AppState>, dir_path: String) -> CommandResult<()> {
    state.core.add_source_dir(&dir_path).await.map_err(map_err)
}

#[tauri::command]
pub async fn booksource_remove_dir(
    state: State<'_, AppState>,
    dir_path: String,
) -> CommandResult<()> {
    state
        .core
        .remove_source_dir(&dir_path)
        .await
        .map_err(map_err)
}

#[tauri::command]
pub async fn booksource_pick_dir(app: tauri::AppHandle) -> CommandResult<String> {
    #[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
    {
        let folder = tokio::task::spawn_blocking(move || app.dialog().file().blocking_pick_folder())
            .await
            .map_err(|err| CommandError {
                code: "IO_ERROR".to_string(),
                message: format!("选择目录失败: {err}"),
                detail: Some(format!("{err:?}")),
                retryable: false,
            })?;
        return Ok(folder
            .and_then(|path| path.into_path().ok())
            .map(|path| path.to_string_lossy().to_string())
            .unwrap_or_default());
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        let _ = app;
        Err(CommandError {
            code: "UNSUPPORTED".to_string(),
            message: "当前平台不支持选择书源目录".to_string(),
            detail: None,
            retryable: false,
        })
    }
}

#[tauri::command]
pub async fn booksource_list(state: State<'_, AppState>) -> CommandResult<Vec<BookSourceMeta>> {
    state.core.list_sources().await.map_err(map_err)
}

#[tauri::command]
pub async fn booksource_list_streaming(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    request_id: String,
) -> CommandResult<()> {
    let items = state.core.list_sources().await.map_err(map_err)?;
    let total = items.len();
    let batch_size = 20;
    for (idx, chunk) in items.chunks(batch_size).enumerate() {
        let done = (idx + 1) * batch_size >= total;
        let _ = app.emit(
            "booksource:batch",
            serde_json::json!({
                "requestId": &request_id,
                "items": chunk,
                "done": done,
                "total": total
            }),
        );
    }
    Ok(())
}

#[tauri::command]
pub async fn booksource_read(
    state: State<'_, AppState>,
    file_name: String,
    source_dir: Option<String>,
) -> CommandResult<String> {
    state
        .core
        .read_source(&file_name, source_dir.as_deref())
        .await
        .map_err(map_err)
}

#[tauri::command]
pub async fn booksource_save(
    state: State<'_, AppState>,
    file_name: String,
    content: String,
    source_dir: Option<String>,
) -> CommandResult<()> {
    state
        .core
        .save_js_source(&file_name, &content, source_dir.as_deref())
        .await
        .map_err(map_err)
}

#[tauri::command]
pub async fn booksource_delete(
    state: State<'_, AppState>,
    file_name: String,
    source_dir: Option<String>,
) -> CommandResult<()> {
    state
        .core
        .delete_source(&file_name, source_dir.as_deref())
        .await
        .map_err(map_err)
}

#[tauri::command]
pub async fn booksource_delete_batch(
    state: State<'_, AppState>,
    items: Vec<DeleteItem>,
) -> CommandResult<DeleteBatchResult> {
    let mut deleted = Vec::new();
    let mut errors = Vec::new();
    for item in items {
        match state
            .core
            .delete_source(&item.file_name, item.source_dir.as_deref())
            .await
        {
            Ok(()) => deleted.push(item),
            Err(err) => errors.push(DeleteError {
                file_name: item.file_name,
                source_dir: item.source_dir,
                message: err.to_string(),
            }),
        }
    }
    Ok(DeleteBatchResult { deleted, errors })
}

#[tauri::command]
pub async fn booksource_toggle(
    state: State<'_, AppState>,
    file_name: String,
    enabled: bool,
    source_dir: Option<String>,
) -> CommandResult<()> {
    state
        .core
        .toggle_source(&file_name, enabled, source_dir.as_deref())
        .await
        .map_err(map_err)
}

#[tauri::command]
pub async fn booksource_import_legacy_json_text(
    state: State<'_, AppState>,
    content: String,
    smart_explore_sub_categories: bool,
) -> CommandResult<LegacyJsonImportResult> {
    state
        .core
        .import_legacy_json_text(&content, smart_explore_sub_categories)
        .await
        .map_err(map_err)
}

#[tauri::command]
pub async fn booksource_import_legacy_json_url(
    state: State<'_, AppState>,
    url: String,
    smart_explore_sub_categories: bool,
) -> CommandResult<LegacyJsonImportResult> {
    state
        .core
        .import_legacy_json_url(&url, smart_explore_sub_categories)
        .await
        .map_err(map_err)
}

#[tauri::command]
pub async fn booksource_eval(
    state: State<'_, AppState>,
    file_name: String,
    entry_code: Option<String>,
    source_dir: Option<String>,
) -> CommandResult<String> {
    let code = entry_code.as_deref().unwrap_or("").trim().to_string();
    if code.is_empty() {
        return state
            .core
            .eval_source_capabilities(&file_name, source_dir.as_deref())
            .await
            .map_err(map_err);
    }
    state
        .core
        .eval_source_entry(&file_name, &code, source_dir.as_deref())
        .await
        .map_err(map_err)
}

#[tauri::command]
pub async fn booksource_save_draft(
    state: State<'_, AppState>,
    file_name: String,
    content: String,
) -> CommandResult<()> {
    state.core.save_draft(&file_name, &content).await.map_err(map_err)
}

#[tauri::command]
pub async fn booksource_search(
    state: State<'_, AppState>,
    file_name: String,
    keyword: String,
    page: i32,
    source_dir: Option<String>,
) -> CommandResult<Vec<BookItem>> {
    state
        .core
        .search(&file_name, &keyword, page, source_dir.as_deref())
        .await
        .map_err(map_err)
}

#[tauri::command]
pub async fn booksource_book_info(
    state: State<'_, AppState>,
    file_name: String,
    book_url: String,
    source_dir: Option<String>,
) -> CommandResult<BookDetail> {
    state
        .core
        .book_info(&file_name, &book_url, source_dir.as_deref())
        .await
        .map_err(map_err)
}

#[tauri::command]
pub async fn booksource_chapter_list(
    state: State<'_, AppState>,
    file_name: String,
    book_url: String,
    _task_id: Option<String>,
    source_dir: Option<String>,
) -> CommandResult<Vec<ChapterItem>> {
    state
        .core
        .chapter_list(&file_name, &book_url, source_dir.as_deref())
        .await
        .map_err(map_err)
}

#[tauri::command]
pub async fn booksource_chapter_content(
    state: State<'_, AppState>,
    file_name: String,
    chapter_url: String,
    source_dir: Option<String>,
    _category_params: Option<Value>,
) -> CommandResult<String> {
    state
        .core
        .chapter_content(&file_name, &chapter_url, source_dir.as_deref())
        .await
        .map_err(map_err)
}

#[tauri::command]
pub async fn booksource_purchase_chapter() -> CommandResult<Value> {
    Ok(serde_json::json!({ "ok": true, "purchased": true }))
}

#[tauri::command]
pub async fn booksource_explore(
    state: State<'_, AppState>,
    file_name: String,
    page: i32,
    category: String,
    _no_cache: Option<bool>,
    source_dir: Option<String>,
) -> CommandResult<Value> {
    state
        .core
        .explore(&file_name, page, &category, source_dir.as_deref())
        .await
        .map_err(map_err)
}

#[tauri::command]
pub async fn booksource_call_fn() -> CommandResult<Value> {
    Err(CommandError {
        code: "UNSUPPORTED".to_string(),
        message: "Route B 原生 Legado 运行时不支持 JS 自定义函数调用".to_string(),
        detail: None,
        retryable: false,
    })
}

#[tauri::command]
pub async fn booksource_cancel(state: State<'_, AppState>, task_id: String) -> CommandResult<()> {
    if state.tasks.cancel(&task_id) {
        Ok(())
    } else {
        Err(CommandError {
            code: "TASK_NOT_FOUND".to_string(),
            message: format!("任务 {} 不存在或已完成", task_id),
            detail: None,
            retryable: false,
        })
    }
}

#[tauri::command]
pub async fn booksource_run_tests(
    state: State<'_, AppState>,
    file_name: String,
    timeout_secs: Option<i32>,
    step_filter: Option<String>,
    source_dir: Option<String>,
) -> CommandResult<Value> {
    let _ = (timeout_secs, step_filter);
    state
        .core
        .run_source_tests(&file_name, source_dir.as_deref())
        .await
        .map_err(map_err)
}
