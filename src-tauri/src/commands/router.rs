//! WS 命令路由（R-P2-008 阶段 1：命令路由收口）
//!
//! Tauri IPC 与 WebSocket 共用同一份命令实现：`#[tauri::command]` 宏保留原函数，
//! 本路由通过 `app.state::<AppState>()` 取状态后直接调用这些函数，不复制命令体。
//!
//! match 分支即命令白名单：未在此列出的命令一律拒绝（含 `js_eval` 与依赖系统对话框 /
//! 资源管理器 / 窗口 / 本机 WebView 的桌面独占命令）。扩白名单时必须确认该命令在
//! 形态 B（远端无头服务）下语义成立，见 docs/frontend-backend-separation.md 第 4 节。
//!
//! 参数键名约定与 Tauri IPC 一致：Rust 参数 snake_case 对应前端 camelCase 键。

use reader_core::{AddBookPayload, CachedChapter, UpdateShelfBookPayload};
use serde::de::DeserializeOwned;
use serde_json::Value;
use tauri::Manager;

use super::bookshelf::{self, ExportBookDataRequest, PrefetchPayload};
use super::config;
use super::source::{self, DeleteItem, HttpProxyRequest};
use super::system;
use crate::state::AppState;

/// 解析 invoke 参数对象到命令参数结构（camelCase 键，缺失必填项即报错，与 Tauri IPC 行为一致）
fn parse_args<T: DeserializeOwned>(raw: &Value) -> Result<T, String> {
    serde_json::from_value(raw.clone()).map_err(|e| format!("INVALID_ARGS: {e}"))
}

/// 命令结果统一序列化：Ok → JSON 值；Err → CommandError 的 JSON 字符串（与 Tauri 拒绝值同构）
fn reply<T: serde::Serialize>(
    result: Result<T, reader_core::CommandError>,
) -> Result<Value, String> {
    match result {
        Ok(v) => serde_json::to_value(v).map_err(|e| format!("SERIALIZE_ERROR: {e}")),
        Err(e) => Err(serde_json::to_string(&e).unwrap_or_else(|_| "COMMAND_ERROR".to_string())),
    }
}

macro_rules! parsed {
    ($raw:expr, { $($field:ident : $ty:ty),* $(,)? }) => {{
        #[derive(serde::Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Args { $($field: $ty),* }
        let Args { $($field),* } = parse_args::<Args>($raw)?;
        ($($field),*)
    }};
}

/// WS 路由命令分发入口（`cmd + args(JSON) → result(JSON)`）
pub async fn dispatch<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    cmd: &str,
    raw: &Value,
) -> Result<Value, String> {
    let state = app.state::<AppState>();
    match cmd {
        // ── system ─────────────────────────────────────────────────────────
        "frontend_log" => {
            let (level, message) = parsed!(raw, { level: String, message: String });
            system::frontend_log(level, message);
            Ok(Value::Null)
        }
        "get_platform" => Ok(Value::String(system::get_platform().to_string())),
        "capabilities_get" => reply(Ok(system::capabilities_get())),

        // ── config / storage ───────────────────────────────────────────────
        "config_read" => {
            let (scope, key) = parsed!(raw, { scope: String, key: String });
            reply(config::config_read(state, scope, key).await)
        }
        "config_write" => {
            let (scope, key, value) = parsed!(raw, { scope: String, key: String, value: String });
            reply(config::config_write(state, scope, key, value).await)
        }
        "config_read_json" => {
            let (scope, key) = parsed!(raw, { scope: String, key: String });
            reply(config::config_read_json(state, scope, key).await)
        }
        "config_write_json" => {
            let (scope, key, value) = parsed!(raw, { scope: String, key: String, value: Value });
            reply(config::config_write_json(state, scope, key, value).await)
        }
        "config_delete_key" => {
            let (scope, key) = parsed!(raw, { scope: String, key: String });
            reply(config::config_delete_key(state, scope, key).await)
        }
        "config_read_all" => {
            let scope = parsed!(raw, { scope: String });
            reply(config::config_read_all(state, scope).await)
        }
        "config_clear" => {
            let scope = parsed!(raw, { scope: String });
            reply(config::config_clear(state, scope).await)
        }
        "config_read_bytes" => {
            let (scope, key) = parsed!(raw, { scope: String, key: String });
            reply(config::config_read_bytes(state, scope, key).await)
        }
        "config_write_bytes" => {
            let (scope, key, value) = parsed!(raw, { scope: String, key: String, value: Vec<u8> });
            reply(config::config_write_bytes(state, scope, key, value).await)
        }
        "config_list_scopes" => reply(config::config_list_scopes(state).await),
        "config_dump_scope" => {
            let scope = parsed!(raw, { scope: String });
            reply(config::config_dump_scope(state, scope).await)
        }
        "frontend_storage_list" => {
            let namespace = parsed!(raw, { namespace: String });
            reply(config::frontend_storage_list(state, namespace).await)
        }
        "frontend_storage_set" => {
            let (namespace, key, value) =
                parsed!(raw, { namespace: String, key: String, value: String });
            reply(config::frontend_storage_set(state, namespace, key, value).await)
        }
        "frontend_storage_remove" => {
            let (namespace, key) = parsed!(raw, { namespace: String, key: String });
            reply(config::frontend_storage_remove(state, namespace, key).await)
        }
        "frontend_storage_list_namespaces" => {
            reply(config::frontend_storage_list_namespaces(state).await)
        }
        "app_config_get_all" => reply(config::app_config_get_all(state).await),
        "app_config_set" => {
            let (key, value) = parsed!(raw, { key: String, value: Value });
            reply(config::app_config_set_impl(app, state.inner(), key, value).await)
        }
        "app_config_reset" => {
            let key = parsed!(raw, { key: String });
            reply(config::app_config_reset_impl(app, state.inner(), key).await)
        }
        "storage_debug_dump" => reply(config::storage_debug_dump(state).await),

        // ── booksource ─────────────────────────────────────────────────────
        "booksource_get_dir" => reply(source::booksource_get_dir(state).await),
        "booksource_get_dirs" => reply(source::booksource_get_dirs(state).await),
        "booksource_list" => reply(source::booksource_list(state).await),
        "booksource_read" => {
            let (file_name, source_dir) =
                parsed!(raw, { file_name: String, source_dir: Option<String> });
            reply(source::booksource_read(state, file_name, source_dir).await)
        }
        "booksource_save" => {
            let (file_name, content, source_dir) =
                parsed!(raw, { file_name: String, content: String, source_dir: Option<String> });
            reply(source::booksource_save(state, file_name, content, source_dir).await)
        }
        "booksource_delete" => {
            let (file_name, source_dir) =
                parsed!(raw, { file_name: String, source_dir: Option<String> });
            reply(source::booksource_delete(state, file_name, source_dir).await)
        }
        "booksource_delete_batch" => {
            let items = parsed!(raw, { items: Vec<DeleteItem> });
            reply(source::booksource_delete_batch(state, items).await)
        }
        "booksource_toggle" => {
            let (file_name, enabled, source_dir) =
                parsed!(raw, { file_name: String, enabled: bool, source_dir: Option<String> });
            reply(source::booksource_toggle(state, file_name, enabled, source_dir).await)
        }
        "booksource_import_legacy_json_text" => {
            let (content, smart_explore_sub_categories) =
                parsed!(raw, { content: String, smart_explore_sub_categories: bool });
            reply(
                source::booksource_import_legacy_json_text(
                    state,
                    content,
                    smart_explore_sub_categories,
                )
                .await,
            )
        }
        "booksource_import_legacy_json_url" => {
            let (url, smart_explore_sub_categories) =
                parsed!(raw, { url: String, smart_explore_sub_categories: bool });
            reply(
                source::booksource_import_legacy_json_url(state, url, smart_explore_sub_categories)
                    .await,
            )
        }
        "booksource_save_draft" => {
            let (file_name, content) = parsed!(raw, { file_name: String, content: String });
            reply(source::booksource_save_draft(state, file_name, content).await)
        }
        "booksource_delete_draft" => {
            let file_name = parsed!(raw, { file_name: String });
            reply(source::booksource_delete_draft(state, file_name).await)
        }
        "booksource_search" => {
            let (file_name, keyword, page, source_dir) = parsed!(raw, {
                file_name: String,
                keyword: String,
                page: i32,
                source_dir: Option<String>,
            });
            reply(source::booksource_search(state, file_name, keyword, page, source_dir).await)
        }
        "booksource_book_info" => {
            let (file_name, book_url, source_dir) = parsed!(raw, {
                file_name: String,
                book_url: String,
                source_dir: Option<String>,
            });
            reply(source::booksource_book_info(state, file_name, book_url, source_dir).await)
        }
        "booksource_chapter_list" => {
            let (file_name, book_url, task_id, source_dir) = parsed!(raw, {
                file_name: String,
                book_url: String,
                task_id: Option<String>,
                source_dir: Option<String>,
            });
            reply(
                source::booksource_chapter_list(state, file_name, book_url, task_id, source_dir)
                    .await,
            )
        }
        "booksource_chapter_content" => {
            let (file_name, chapter_url, source_dir, task_id) = parsed!(raw, {
                file_name: String,
                chapter_url: String,
                source_dir: Option<String>,
                task_id: Option<String>,
            });
            reply(
                source::booksource_chapter_content(
                    state,
                    file_name,
                    chapter_url,
                    source_dir,
                    None,
                    task_id,
                )
                .await,
            )
        }
        "booksource_purchase_chapter" => {
            let (file_name, chapter_url, chapter, source_dir) = parsed!(raw, {
                file_name: String,
                chapter_url: String,
                chapter: Option<Value>,
                source_dir: Option<String>,
            });
            reply(
                source::booksource_purchase_chapter(
                    state,
                    file_name,
                    chapter_url,
                    chapter,
                    source_dir,
                )
                .await,
            )
        }
        "booksource_explore" => {
            let (file_name, page, category, source_dir) = parsed!(raw, {
                file_name: String,
                page: i32,
                category: String,
                source_dir: Option<String>,
            });
            reply(
                source::booksource_explore(state, file_name, page, category, None, source_dir)
                    .await,
            )
        }
        "booksource_call_fn" => {
            let (file_name, fn_name, args, source_dir) = parsed!(raw, {
                file_name: String,
                fn_name: String,
                args: Vec<Value>,
                source_dir: Option<String>,
            });
            reply(source::booksource_call_fn(state, file_name, fn_name, args, source_dir).await)
        }
        "booksource_cancel" => {
            let task_id = parsed!(raw, { task_id: String });
            reply(source::booksource_cancel(state, task_id).await)
        }
        "booksource_http_proxy" => {
            let request = parsed!(raw, { request: HttpProxyRequest });
            reply(source::booksource_http_proxy(state, request).await)
        }

        // ── bookshelf ──────────────────────────────────────────────────────
        "bookshelf_list" => reply(bookshelf::bookshelf_list(state).await),
        "bookshelf_add" => {
            let (book, file_name, source_name) = parsed!(raw, {
                book: AddBookPayload,
                file_name: String,
                source_name: String,
            });
            reply(bookshelf::bookshelf_add(state, book, file_name, source_name).await)
        }
        "bookshelf_remove" => {
            let id = parsed!(raw, { id: String });
            reply(bookshelf::bookshelf_remove(state, id).await)
        }
        "bookshelf_get" => {
            let id = parsed!(raw, { id: String });
            reply(bookshelf::bookshelf_get(state, id).await)
        }
        "bookshelf_update_progress" => {
            let (id, chapter_index, chapter_url, page_index, scroll_ratio, playback_time, reader_settings) =
                parsed!(raw, {
                    id: String,
                    chapter_index: i32,
                    chapter_url: String,
                    page_index: Option<i32>,
                    scroll_ratio: Option<f64>,
                    playback_time: Option<f64>,
                    reader_settings: Option<String>,
                });
            reply(
                bookshelf::bookshelf_update_progress(
                    state,
                    id,
                    chapter_index,
                    chapter_url,
                    page_index,
                    scroll_ratio,
                    playback_time,
                    reader_settings,
                )
                .await,
            )
        }
        "bookshelf_set_private" => {
            let (id, is_private) = parsed!(raw, { id: String, is_private: bool });
            reply(bookshelf::bookshelf_set_private(state, id, is_private).await)
        }
        "bookshelf_save_chapters" => {
            let (id, chapters) = parsed!(raw, { id: String, chapters: Vec<CachedChapter> });
            reply(bookshelf::bookshelf_save_chapters(state, id, chapters).await)
        }
        "bookshelf_get_chapters" => {
            let id = parsed!(raw, { id: String });
            reply(bookshelf::bookshelf_get_chapters(state, id).await)
        }
        "bookshelf_update_book" => {
            let (book, chapters) = parsed!(raw, {
                book: UpdateShelfBookPayload,
                chapters: Option<Vec<CachedChapter>>,
            });
            reply(bookshelf::bookshelf_update_book(state, book, chapters).await)
        }
        "bookshelf_restore_source_switch" => {
            let id = parsed!(raw, { id: String });
            reply(bookshelf::bookshelf_restore_source_switch(state, id).await)
        }
        "bookshelf_save_content" => {
            let (id, chapter_index, content) =
                parsed!(raw, { id: String, chapter_index: i32, content: String });
            reply(bookshelf::bookshelf_save_content(state, id, chapter_index, content).await)
        }
        "bookshelf_get_content" => {
            let (id, chapter_index) = parsed!(raw, { id: String, chapter_index: i32 });
            reply(bookshelf::bookshelf_get_content(state, id, chapter_index).await)
        }
        "bookshelf_delete_content" => {
            let (id, chapter_index) = parsed!(raw, { id: String, chapter_index: i32 });
            reply(bookshelf::bookshelf_delete_content(state, id, chapter_index).await)
        }
        "bookshelf_get_cached_indices" => {
            let id = parsed!(raw, { id: String });
            reply(bookshelf::bookshelf_get_cached_indices(state, id).await)
        }
        "bookshelf_save_txt_chapters" => {
            let (id, chapters) = parsed!(raw, { id: String, chapters: Vec<CachedChapter> });
            reply(bookshelf::bookshelf_save_txt_chapters(state, id, chapters).await)
        }
        "bookshelf_get_episode_progress" => {
            let id = parsed!(raw, { id: String });
            reply(bookshelf::bookshelf_get_episode_progress(state, id).await)
        }
        "bookshelf_save_episode_progress" => {
            let (id, chapter_url, time, duration) = parsed!(raw, {
                id: String,
                chapter_url: String,
                time: f64,
                duration: f64,
            });
            reply(
                bookshelf::bookshelf_save_episode_progress(state, id, chapter_url, time, duration)
                    .await,
            )
        }
        "bookshelf_prefetch_chapters" => {
            let payload = parsed!(raw, { payload: PrefetchPayload });
            reply(bookshelf::bookshelf_prefetch_chapters_impl(&state, &payload).await)
        }
        "bookshelf_export_book_data" => {
            let request = parsed!(raw, { request: ExportBookDataRequest });
            reply(bookshelf::bookshelf_export_book_data(state, request).await)
        }

        // ── 拒绝项 ─────────────────────────────────────────────────────────
        "js_eval" => Err("SECURITY_BLOCKED: js_eval 在所有传输方式下均被阻断".to_string()),
        _ => Err(format!(
            "NOT_ROUTED: 命令 {cmd} 未在 WS 白名单中（桌面独占或未迁移，见 docs/frontend-backend-separation.md）"
        )),
    }
}
