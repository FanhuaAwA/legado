use crate::state::AppState;
use reader_core::model::ai_proxy::AiHttpProxyResponse;
use reader_core::{
    BookDetail, BookItem, BookSourceMeta, ChapterItem, CommandError, LegacyJsonImportProgress,
    LegacyJsonImportResult,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::{Emitter, State};
#[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
use tauri_plugin_dialog::DialogExt;

type CommandResult<T> = Result<T, CommandError>;

fn map_err(err: reader_core::ReaderCoreError) -> CommandError {
    err.into_command_error()
}

fn cancelled_error() -> CommandError {
    CommandError {
        code: "CANCELLED".to_string(),
        message: "任务已取消".to_string(),
        detail: None,
        retryable: false,
    }
}

pub fn emit_legacy_import_progress<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    request_id: &str,
    progress: LegacyJsonImportProgress,
) {
    let mut payload = serde_json::to_value(progress).unwrap_or_else(|_| serde_json::json!({}));
    if let Some(object) = payload.as_object_mut() {
        object.insert("requestId".to_string(), serde_json::json!(request_id));
    }
    let _ = app.emit("booksource:import-progress", payload);
}

async fn wait_for_cancel(cancelled: Arc<AtomicBool>) {
    while !cancelled.load(Ordering::SeqCst) {
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
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
        let folder =
            tokio::task::spawn_blocking(move || app.dialog().file().blocking_pick_folder())
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
    force: Option<bool>,
) -> CommandResult<()> {
    let batch_size = 20;
    state
        .core
        .stream_sources(batch_size, force.unwrap_or(false), |items, done, total| {
            let app = app.clone();
            let request_id = request_id.clone();
            async move {
                let _ = app.emit(
                    "booksource:batch",
                    serde_json::json!({
                        "requestId": request_id,
                        "items": items,
                        "done": done,
                        "total": total
                    }),
                );
            }
        })
        .await
        .map(|_| ())
        .map_err(map_err)
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
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    content: String,
    smart_explore_sub_categories: bool,
    request_id: Option<String>,
) -> CommandResult<LegacyJsonImportResult> {
    let request_id = request_id.unwrap_or_default();
    let app_for_progress = app.clone();
    state
        .core
        .import_legacy_json_text_with_progress(
            &content,
            smart_explore_sub_categories,
            move |progress: LegacyJsonImportProgress| {
                let app = app_for_progress.clone();
                let request_id = request_id.clone();
                async move {
                    emit_legacy_import_progress(&app, &request_id, progress);
                }
            },
        )
        .await
        .map_err(map_err)
}

#[tauri::command]
pub async fn booksource_import_legacy_json_url(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    url: String,
    smart_explore_sub_categories: bool,
    request_id: Option<String>,
) -> CommandResult<LegacyJsonImportResult> {
    let request_id = request_id.unwrap_or_default();
    let app_for_progress = app.clone();
    state
        .core
        .import_legacy_json_url_with_progress(
            &url,
            smart_explore_sub_categories,
            move |progress: LegacyJsonImportProgress| {
                let app = app_for_progress.clone();
                let request_id = request_id.clone();
                async move {
                    emit_legacy_import_progress(&app, &request_id, progress);
                }
            },
        )
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
    state
        .core
        .save_draft(&file_name, &content)
        .await
        .map_err(map_err)
}

#[tauri::command]
pub async fn booksource_search(
    state: State<'_, AppState>,
    file_name: String,
    keyword: String,
    page: i32,
    task_id: Option<String>,
    source_dir: Option<String>,
) -> CommandResult<Vec<BookItem>> {
    let token = task_id.as_deref().map(|tid| state.tasks.register(tid));
    if let Some(ref t) = token {
        if t.load(Ordering::SeqCst) {
            return Err(cancelled_error());
        }
    }

    let result = if let Some(t) = token.clone() {
        let search_token = Some(t.clone());
        tokio::select! {
            result = state.core.search_with_cancel(
                &file_name,
                &keyword,
                page,
                source_dir.as_deref(),
                search_token,
            ) => {
                result.map_err(map_err)
            }
            _ = wait_for_cancel(t) => Err(cancelled_error()),
        }
    } else {
        state
            .core
            .search_with_cancel(&file_name, &keyword, page, source_dir.as_deref(), None)
            .await
            .map_err(map_err)
    };
    if let Some(tid) = task_id.as_deref() {
        state.tasks.remove(tid);
    }
    result
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
    task_id: Option<String>,
    source_dir: Option<String>,
) -> CommandResult<Vec<ChapterItem>> {
    let token = task_id.as_deref().map(|tid| state.tasks.register(tid));
    if let Some(ref t) = token {
        if t.load(Ordering::SeqCst) {
            return Err(cancelled_error());
        }
    }
    let result = state
        .core
        .chapter_list(&file_name, &book_url, source_dir.as_deref())
        .await
        .map_err(map_err);
    if let Some(tid) = task_id.as_deref() {
        state.tasks.remove(tid);
    }
    result
}

#[tauri::command]
pub async fn booksource_chapter_content(
    state: State<'_, AppState>,
    file_name: String,
    chapter_url: String,
    source_dir: Option<String>,
    _category_params: Option<Value>,
    task_id: Option<String>,
) -> CommandResult<String> {
    let token = task_id.as_deref().map(|tid| state.tasks.register(tid));
    if let Some(ref t) = token {
        if t.load(Ordering::SeqCst) {
            return Err(cancelled_error());
        }
    }
    let result = state
        .core
        .chapter_content(&file_name, &chapter_url, source_dir.as_deref())
        .await
        .map_err(map_err);
    if let Some(tid) = task_id.as_deref() {
        state.tasks.remove(tid);
    }
    result
}

#[tauri::command]
pub async fn booksource_purchase_chapter(
    state: State<'_, AppState>,
    file_name: String,
    chapter_url: String,
    chapter: Option<Value>,
    source_dir: Option<String>,
) -> CommandResult<Value> {
    state
        .core
        .purchase_chapter(
            &file_name,
            &chapter_url,
            chapter.as_ref(),
            source_dir.as_deref(),
        )
        .await
        .map_err(map_err)
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
pub async fn booksource_call_fn(
    state: State<'_, AppState>,
    file_name: String,
    fn_name: String,
    args: Vec<Value>,
    source_dir: Option<String>,
) -> CommandResult<Value> {
    state
        .core
        .source_call_fn(&file_name, &fn_name, args, source_dir.as_deref())
        .await
        .map_err(map_err)
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
    state
        .core
        .run_source_tests(
            &file_name,
            source_dir.as_deref(),
            step_filter.as_deref(),
            timeout_secs,
        )
        .await
        .map_err(map_err)
}

#[tauri::command]
pub fn booksource_resolve_path(
    state: State<'_, AppState>,
    file_name: String,
    source_dir: Option<String>,
) -> CommandResult<String> {
    state
        .core
        .resolve_source_path(&file_name, source_dir.as_deref())
        .map(|p| p.to_string_lossy().to_string())
        .map_err(map_err)
}

#[tauri::command]
pub fn booksource_open_in_vscode(
    state: State<'_, AppState>,
    file_name: String,
    source_dir: Option<String>,
) -> CommandResult<()> {
    let path = state
        .core
        .resolve_source_path(&file_name, source_dir.as_deref())
        .map_err(map_err)?;
    tauri_plugin_opener::open_path(path.to_string_lossy().to_string(), Some("code"))
        .or_else(|_| {
            tauri_plugin_opener::open_path(path.to_string_lossy().to_string(), None::<&str>)
        })
        .map_err(|err| CommandError {
            code: "IO_ERROR".to_string(),
            message: err.to_string(),
            detail: Some(format!("{err:?}")),
            retryable: false,
        })
}

#[tauri::command]
pub async fn booksource_delete_draft(
    state: State<'_, AppState>,
    file_name: String,
) -> CommandResult<()> {
    state.core.delete_draft(&file_name).await.map_err(map_err)
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HttpProxyRequest {
    pub url: String,
    pub method: String,
    pub body: Option<String>,
    pub headers: Option<Vec<String>>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiHttpProxyRequest {
    pub url: String,
    pub method: String,
    pub body: Option<String>,
    pub headers: Option<Vec<String>>,
}

#[tauri::command]
pub async fn ai_http_proxy_request(
    state: State<'_, AppState>,
    request: AiHttpProxyRequest,
) -> CommandResult<AiHttpProxyResponse> {
    let body = request.body.as_deref();
    let headers: Option<Vec<String>> = request.headers;
    let headers_ref: Option<&[String]> = headers.as_deref();
    state
        .core
        .ai_proxy_request(&request.url, &request.method, body, headers_ref)
        .await
        .map_err(map_err)
}

#[tauri::command]
pub async fn booksource_http_proxy(
    state: State<'_, AppState>,
    request: HttpProxyRequest,
) -> CommandResult<String> {
    // Security: only allow http/https
    if !request.url.starts_with("http://") && !request.url.starts_with("https://") {
        return Err(CommandError {
            code: "BLOCKED".to_string(),
            message: "仅支持 http/https 协议".to_string(),
            detail: None,
            retryable: false,
        });
    }
    // Security: block common internal addresses
    let lower = request.url.to_lowercase();
    if lower.contains("127.0.0.1")
        || lower.contains("localhost")
        || lower.contains("0.0.0.0")
        || lower.contains("::1")
        || lower.contains("169.254.")
        || lower.contains("10.")
        || lower.contains("172.16.")
        || lower.contains("192.168.")
    {
        return Err(CommandError {
            code: "BLOCKED".to_string(),
            message: "禁止访问内网地址".to_string(),
            detail: None,
            retryable: false,
        });
    }
    let body = request.body.as_deref();
    let headers: Option<Vec<String>> = request.headers;
    let headers_ref: Option<&[String]> = headers.as_deref();
    state
        .core
        .http_proxy_request(&request.url, &request.method, body, headers_ref)
        .await
        .map_err(map_err)
}
