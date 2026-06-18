use crate::state::AppState;
use reader_core::{
    AddBookPayload, CachedChapter, CommandError, EpisodeProgressMap, ShelfBook,
    SourceSwitchRestoreResult, UpdateShelfBookPayload,
};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::{Emitter, State};

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

fn normalize_cancelled_result<T>(
    result: CommandResult<T>,
    token: Option<&Arc<AtomicBool>>,
) -> CommandResult<T> {
    if token
        .map(|token| token.load(Ordering::SeqCst))
        .unwrap_or(false)
    {
        result.map_err(|_| cancelled_error())
    } else {
        result
    }
}

#[tauri::command]
pub async fn bookshelf_list(state: State<'_, AppState>) -> CommandResult<Vec<ShelfBook>> {
    state.core.shelf_list().await.map_err(map_err)
}

#[tauri::command]
pub async fn bookshelf_add(
    state: State<'_, AppState>,
    book: AddBookPayload,
    file_name: String,
    source_name: String,
) -> CommandResult<ShelfBook> {
    state
        .core
        .shelf_add(book, &file_name, &source_name)
        .await
        .map_err(map_err)
}

#[tauri::command]
pub async fn bookshelf_remove(state: State<'_, AppState>, id: String) -> CommandResult<()> {
    state.core.shelf_remove(&id).await.map_err(map_err)
}

#[tauri::command]
pub async fn bookshelf_get(state: State<'_, AppState>, id: String) -> CommandResult<ShelfBook> {
    state.core.shelf_get(&id).await.map_err(map_err)
}

#[tauri::command]
pub async fn bookshelf_update_progress(
    state: State<'_, AppState>,
    id: String,
    chapter_index: i32,
    chapter_url: String,
    page_index: Option<i32>,
    scroll_ratio: Option<f64>,
    playback_time: Option<f64>,
    reader_settings: Option<String>,
) -> CommandResult<()> {
    state
        .core
        .shelf_update_progress(
            &id,
            chapter_index,
            &chapter_url,
            page_index,
            scroll_ratio,
            playback_time,
            reader_settings,
        )
        .await
        .map_err(map_err)
}

#[tauri::command]
pub async fn bookshelf_set_private(
    state: State<'_, AppState>,
    id: String,
    is_private: bool,
) -> CommandResult<()> {
    state
        .core
        .shelf_set_private(&id, is_private)
        .await
        .map_err(map_err)
}

#[tauri::command]
pub async fn bookshelf_save_chapters(
    state: State<'_, AppState>,
    id: String,
    chapters: Vec<CachedChapter>,
) -> CommandResult<()> {
    state
        .core
        .shelf_save_chapters(&id, chapters)
        .await
        .map_err(map_err)
}

#[tauri::command]
pub async fn bookshelf_get_chapters(
    state: State<'_, AppState>,
    id: String,
) -> CommandResult<Vec<CachedChapter>> {
    state.core.shelf_get_chapters(&id).await.map_err(map_err)
}

#[tauri::command]
pub async fn bookshelf_update_book(
    state: State<'_, AppState>,
    book: UpdateShelfBookPayload,
    chapters: Option<Vec<CachedChapter>>,
) -> CommandResult<ShelfBook> {
    state
        .core
        .shelf_update_book(book, chapters)
        .await
        .map_err(map_err)
}

#[tauri::command]
pub async fn bookshelf_restore_source_switch(
    state: State<'_, AppState>,
    id: String,
) -> CommandResult<SourceSwitchRestoreResult> {
    state
        .core
        .shelf_restore_source_switch(&id)
        .await
        .map_err(map_err)
}

#[tauri::command]
pub async fn bookshelf_save_content(
    state: State<'_, AppState>,
    id: String,
    chapter_index: i32,
    content: String,
) -> CommandResult<()> {
    state
        .core
        .shelf_save_content(&id, chapter_index, &content)
        .await
        .map_err(map_err)
}

#[tauri::command]
pub async fn bookshelf_get_content(
    state: State<'_, AppState>,
    id: String,
    chapter_index: i32,
) -> CommandResult<Option<String>> {
    state
        .core
        .shelf_get_content(&id, chapter_index)
        .await
        .map_err(map_err)
}

#[tauri::command]
pub async fn bookshelf_delete_content(
    state: State<'_, AppState>,
    id: String,
    chapter_index: i32,
) -> CommandResult<()> {
    state
        .core
        .shelf_delete_content(&id, chapter_index)
        .await
        .map_err(map_err)
}

#[tauri::command]
pub async fn bookshelf_get_cached_indices(
    state: State<'_, AppState>,
    id: String,
) -> CommandResult<Vec<i32>> {
    state.core.shelf_cached_indices(&id).await.map_err(map_err)
}

#[tauri::command]
pub async fn bookshelf_save_txt_chapters(
    state: State<'_, AppState>,
    id: String,
    chapters: Vec<CachedChapter>,
) -> CommandResult<()> {
    state
        .core
        .shelf_save_chapters(&id, chapters)
        .await
        .map_err(map_err)
}

#[tauri::command]
pub async fn bookshelf_get_episode_progress(
    state: State<'_, AppState>,
    id: String,
) -> CommandResult<EpisodeProgressMap> {
    state
        .core
        .shelf_get_episode_progress(&id)
        .await
        .map_err(map_err)
}

#[tauri::command]
pub async fn bookshelf_save_episode_progress(
    state: State<'_, AppState>,
    id: String,
    chapter_url: String,
    time: f64,
    duration: f64,
) -> CommandResult<()> {
    state
        .core
        .shelf_save_episode_progress(&id, &chapter_url, time, duration)
        .await
        .map_err(map_err)
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrefetchPayload {
    pub id: String,
    pub file_name: String,
    pub source_dir: Option<String>,
    pub task_id: String,
    /// 起始章节下标；None 时从头开始（兼容旧调用方）。
    pub start_index: Option<i32>,
    /// 向后缓存章节数；None 或负数表示缓存到书末。
    pub count: Option<i32>,
}

#[tauri::command]
pub async fn bookshelf_prefetch_chapters(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    payload: PrefetchPayload,
) -> CommandResult<i32> {
    let tid = payload.task_id.clone();
    let app_for_progress = app.clone();
    let on_progress = move |done: i32, total: i32, chapter_index: i32| {
        let _ = app_for_progress.emit(
            "shelf:prefetch-progress",
            serde_json::json!({
                "taskId": tid,
                "done": done,
                "total": total,
                "chapterIndex": chapter_index,
            }),
        );
    };
    let result = bookshelf_prefetch_chapters_impl(&state, &payload, Some(on_progress)).await;
    // Emit done event.
    let _ = app.emit(
        "shelf:prefetch-done",
        serde_json::json!({
            "taskId": payload.task_id,
            "error": result.as_ref().err().map(|e| format!("{e:?}")),
        }),
    );
    result
}

/// Shared implementation (WS router calls this — no AppHandle, no progress).
pub async fn bookshelf_prefetch_chapters_impl<F>(
    state: &State<'_, AppState>,
    payload: &PrefetchPayload,
    on_progress: Option<F>,
) -> CommandResult<i32>
where
    F: Fn(i32, i32, i32) + Send + Sync + 'static,
{
    let cancelled = state.tasks.register(&payload.task_id);
    let result = state
        .core
        .prefetch_chapters(
            &payload.id,
            &payload.file_name,
            payload.source_dir.as_deref(),
            payload.start_index,
            payload.count,
            Some(cancelled.clone()),
            on_progress,
        )
        .await
        .map_err(map_err);
    let result = normalize_cancelled_result(result, Some(&cancelled));
    state.tasks.remove_if_current(&payload.task_id, &cancelled);
    result
}

#[tauri::command]
pub async fn bookshelf_pick_save_path(
    app: tauri::AppHandle,
    default_name: String,
    filter_name: String,
    filter_exts: Vec<String>,
) -> CommandResult<Option<String>> {
    #[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
    {
        use tauri_plugin_dialog::DialogExt;
        let exts: Vec<&str> = filter_exts.iter().map(|s| s.as_str()).collect();
        let result = app
            .dialog()
            .file()
            .add_filter(&filter_name, &exts)
            .set_file_name(&default_name)
            .blocking_save_file();
        Ok(result.map(|p| p.to_string()))
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        let _ = (app, default_name, filter_name, filter_exts);
        Err(CommandError {
            code: "UNSUPPORTED".to_string(),
            message: "文件保存路径选择仅支持桌面端".to_string(),
            detail: None,
            retryable: false,
        })
    }
}

#[tauri::command]
pub async fn bookshelf_reveal_data_dir(
    state: State<'_, AppState>,
    _id: String,
) -> CommandResult<()> {
    let reader_dir = state.core.reader_dir().to_string_lossy().to_string();
    tauri_plugin_opener::open_path(reader_dir, None::<&str>).map_err(|err| CommandError {
        code: "IO_ERROR".to_string(),
        message: err.to_string(),
        detail: Some(format!("{err:?}")),
        retryable: false,
    })
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportSaveFileRequest {
    pub default_name: String,
    pub mime: String,
    pub text: String,
    pub base64: String,
    pub extensions: Vec<String>,
}

#[tauri::command]
pub async fn export_save_file(
    app: tauri::AppHandle,
    request: ExportSaveFileRequest,
) -> CommandResult<Option<String>> {
    use base64::Engine as _;
    use tauri_plugin_dialog::DialogExt;
    use tokio::fs;
    let exts: Vec<&str> = request.extensions.iter().map(|s| s.as_str()).collect();
    let result = app
        .dialog()
        .file()
        .add_filter(&request.mime, if exts.is_empty() { &["*"] } else { &exts })
        .set_file_name(&request.default_name)
        .blocking_save_file();
    match result {
        Some(path) => {
            let path_str = path.to_string();
            if !request.base64.is_empty() {
                let bytes = base64::engine::general_purpose::STANDARD
                    .decode(&request.base64)
                    .map_err(|err| CommandError {
                        code: "IO_ERROR".to_string(),
                        message: format!("base64 decode failed: {err}"),
                        detail: None,
                        retryable: false,
                    })?;
                fs::write(&path_str, &bytes)
                    .await
                    .map_err(|err| CommandError {
                        code: "IO_ERROR".to_string(),
                        message: err.to_string(),
                        detail: None,
                        retryable: false,
                    })?;
            } else {
                fs::write(&path_str, &request.text)
                    .await
                    .map_err(|err| CommandError {
                        code: "IO_ERROR".to_string(),
                        message: err.to_string(),
                        detail: None,
                        retryable: false,
                    })?;
            }
            Ok(Some(path_str))
        }
        None => Ok(None),
    }
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportBookRequest {
    pub id: String,
    pub format: String,
    pub save_path: String,
}

#[tauri::command]
pub async fn bookshelf_export_book(
    state: State<'_, AppState>,
    request: ExportBookRequest,
) -> CommandResult<()> {
    state
        .core
        .export_book(&request.id, &request.format, &request.save_path)
        .await
        .map_err(map_err)
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportBookDataRequest {
    pub id: String,
    pub format: String,
}

#[tauri::command]
pub async fn bookshelf_export_book_data(
    state: State<'_, AppState>,
    request: ExportBookDataRequest,
) -> CommandResult<serde_json::Value> {
    let data = state
        .core
        .export_book_data(&request.id, &request.format)
        .await
        .map_err(map_err)?;
    Ok(data)
}

#[tauri::command]
pub fn bookshelf_reveal_export_file(path: String) -> CommandResult<()> {
    let p = std::path::Path::new(&path);
    if let Some(parent) = p.parent() {
        tauri_plugin_opener::open_path(parent.to_string_lossy().to_string(), None::<&str>).map_err(
            |err| CommandError {
                code: "IO_ERROR".to_string(),
                message: err.to_string(),
                detail: Some(format!("{err:?}")),
                retryable: false,
            },
        )
    } else {
        Err(CommandError {
            code: "BAD_REQUEST".to_string(),
            message: "无效的文件路径".to_string(),
            detail: None,
            retryable: false,
        })
    }
}
