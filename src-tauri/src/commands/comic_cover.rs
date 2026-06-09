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

/// 获取漫画各页的 [width, height]。漫画缓存系统尚未实现，返回结构化 UNSUPPORTED。
/// 前端 ComisMode 应据此隐藏页面尺寸相关 UI。
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct ComicGetPageSizesRequest {
    pub file_name: String,
    pub book_url: String,
    pub book_name: String,
    pub chapter_index: i32,
}
#[tauri::command]
pub async fn comic_get_page_sizes(_req: ComicGetPageSizesRequest) -> CommandResult<Vec<Option<[u32; 2]>>> {
    Err(CommandError {
        code: "UNSUPPORTED".into(),
        message: "漫画缓存系统尚未实现，无法获取页面尺寸。请将漫画缓存模块标记为 unsupported_hidden。".into(),
        detail: Some("comic_cache_not_implemented".into()),
        retryable: false,
    })
}

// ── 封面缓存 ──────────────────────────────────────────────
#[tauri::command] pub async fn cover_cache_clear() -> CommandResult<()> { Err(unsupported("封面缓存清理")) }
#[tauri::command] pub async fn cover_cache_size() -> CommandResult<()> { Err(unsupported("封面缓存大小")) }
#[tauri::command] pub async fn cover_resolve_cache() -> CommandResult<()> { Err(unsupported("封面缓存解析")) }
