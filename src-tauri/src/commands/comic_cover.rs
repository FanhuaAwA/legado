use reader_core::CommandError;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tauri::State;

use crate::state::AppState;

type CommandResult<T> = Result<T, CommandError>;

fn unsupported(feature: &str) -> CommandError {
    CommandError {
        code: "UNSUPPORTED".to_string(),
        message: format!("{feature} is not implemented in this build"),
        detail: None,
        retryable: false,
    }
}

// Comic page cache remains platform-blocked. Cover cache below is a separate,
// transport-safe disk cache used by normal book/search/explore cover images.
#[tauri::command]
pub async fn comic_cache_clear() -> CommandResult<()> {
    Err(unsupported("comic cache clear"))
}

#[tauri::command]
pub async fn comic_cache_clear_chapter() -> CommandResult<()> {
    Err(unsupported("comic chapter cache clear"))
}

#[tauri::command]
pub async fn comic_cache_size() -> CommandResult<()> {
    Err(unsupported("comic cache size"))
}

#[tauri::command]
pub async fn comic_download_images() -> CommandResult<()> {
    Err(unsupported("comic image download"))
}

#[tauri::command]
pub async fn comic_get_cached_page() -> CommandResult<()> {
    Err(unsupported("comic cached page"))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct ComicGetPageSizesRequest {
    pub file_name: String,
    pub book_url: String,
    pub book_name: String,
    pub chapter_index: i32,
}

#[tauri::command]
pub async fn comic_get_page_sizes(
    _req: ComicGetPageSizesRequest,
) -> CommandResult<Vec<Option<[u32; 2]>>> {
    Err(CommandError {
        code: "UNSUPPORTED".into(),
        message: "Comic page cache is not implemented in this build.".into(),
        detail: Some("comic_cache_not_implemented".into()),
        retryable: false,
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CoverResolveRequest {
    pub url: String,
    pub referer: Option<String>,
    pub headers: Option<HashMap<String, String>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CoverResolveRequestWrapper {
    pub request: CoverResolveRequest,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CoverResolveResponse {
    pub local_path: String,
    pub local_ref: String,
}

#[tauri::command]
pub async fn cover_cache_clear(state: State<'_, AppState>) -> CommandResult<u64> {
    state
        .core
        .clear_cover_cache()
        .await
        .map_err(|err| err.into_command_error())
}

#[tauri::command]
pub async fn cover_cache_size(state: State<'_, AppState>) -> CommandResult<u64> {
    state
        .core
        .cover_cache_size()
        .await
        .map_err(|err| err.into_command_error())
}

#[tauri::command]
pub async fn cover_resolve_cache(
    state: State<'_, AppState>,
    request: CoverResolveRequestWrapper,
) -> CommandResult<CoverResolveResponse> {
    let local_path = state
        .core
        .resolve_cover_cache(
            &request.request.url,
            request.request.referer.as_deref(),
            request.request.headers.as_ref(),
        )
        .await
        .map_err(|err| err.into_command_error())?;
    Ok(CoverResolveResponse {
        local_ref: format!("local://{local_path}"),
        local_path,
    })
}
