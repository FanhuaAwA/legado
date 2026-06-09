use reader_core::CommandError;

type CommandResult<T> = Result<T, CommandError>;

fn unsupported(feature: &str) -> CommandError {
    CommandError {
        code: "UNSUPPORTED".to_string(),
        message: format!("{feature} 功能尚未实现"),
        detail: None,
        retryable: false,
    }
}

// ── 漫画缓存 ──────────────────────────────────────────────
#[tauri::command] pub async fn comic_cache_clear() -> CommandResult<()> { Err(unsupported("漫画缓存清理")) }
#[tauri::command] pub async fn comic_cache_clear_chapter() -> CommandResult<()> { Err(unsupported("漫画章节缓存清理")) }
#[tauri::command] pub async fn comic_cache_size() -> CommandResult<()> { Err(unsupported("漫画缓存大小")) }
#[tauri::command] pub async fn comic_download_images() -> CommandResult<()> { Err(unsupported("漫画图片下载")) }
#[tauri::command] pub async fn comic_get_cached_page() -> CommandResult<()> { Err(unsupported("漫画缓存页")) }

// ── 封面缓存 ──────────────────────────────────────────────
#[tauri::command] pub async fn cover_cache_clear() -> CommandResult<()> { Err(unsupported("封面缓存清理")) }
#[tauri::command] pub async fn cover_cache_size() -> CommandResult<()> { Err(unsupported("封面缓存大小")) }
#[tauri::command] pub async fn cover_resolve_cache() -> CommandResult<()> { Err(unsupported("封面缓存解析")) }
