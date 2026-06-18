//! Legado Tauri headless server (R-P2-008 phase 4)
//!
//! Runs reader-core without Tauri: HTTP static file serving + WebSocket command
//! endpoint. Compatible with the useTransport browser client
//! (`?ws=ws://host:7688/ws`).
//!
//! Usage:
//!   legado-headless [--port 7688] [--bind 127.0.0.1] [--dist ./dist]
//!                   [--token my-secret] [--data ./reader-data]
//!
//! WS protocol (same as Tauri WS server):
//!   → { "type": "invoke", "id": "uuid", "cmd": "...", "args": {} }
//!   ← { "type": "response", "id": "uuid", "data": ... }
//!   ← { "type": "response", "id": "uuid", "error": "..." }

use axum::extract::ws::{Message, WebSocket};
use axum::extract::{Path, Query, WebSocketUpgrade};
use axum::http::{header, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use futures::{SinkExt, StreamExt};
use reader_core::{
    AddBookPayload, CachedChapter, ReaderCore, ReaderCoreOptions, UpdateShelfBookPayload,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex as StdMutex};
use tokio::sync::Mutex as TokioMutex;

type WsOutgoing = Arc<TokioMutex<Option<tokio::sync::mpsc::UnboundedSender<String>>>>;

#[derive(Clone, Default)]
struct TaskRegistry {
    tokens: Arc<StdMutex<HashMap<String, Arc<AtomicBool>>>>,
}

impl TaskRegistry {
    fn register(&self, task_id: &str) -> Arc<AtomicBool> {
        let cancelled = Arc::new(AtomicBool::new(false));
        let mut map = self.tokens.lock().unwrap_or_else(|err| err.into_inner());
        if let Some(previous) = map.insert(task_id.to_string(), cancelled.clone()) {
            previous.store(true, Ordering::SeqCst);
        }
        cancelled
    }

    fn cancel(&self, task_id: &str) -> bool {
        let mut map = self.tokens.lock().unwrap_or_else(|err| err.into_inner());
        if let Some(cancelled) = map.remove(task_id) {
            cancelled.store(true, Ordering::SeqCst);
            true
        } else {
            false
        }
    }

    fn remove_if_current(&self, task_id: &str, token: &Arc<AtomicBool>) -> bool {
        let mut map = self.tokens.lock().unwrap_or_else(|err| err.into_inner());
        let is_current = map
            .get(task_id)
            .map(|current| Arc::ptr_eq(current, token))
            .unwrap_or(false);
        if is_current {
            map.remove(task_id);
        }
        is_current
    }
}

#[derive(Clone)]
struct AppState {
    core: Arc<ReaderCore>,
    tasks: TaskRegistry,
    token: Option<String>,
}

#[derive(Deserialize)]
struct WsInbound {
    #[serde(rename = "type")]
    kind: String,
    id: Option<String>,
    cmd: Option<String>,
    args: Option<Value>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info,legado_headless=debug".into()),
        )
        .init();

    let port: u16 = parse_env_or_arg("PORT", "--port", 7688);
    let bind = std::env::var("BIND")
        .ok()
        .unwrap_or_else(|| "127.0.0.1".to_string());
    let dist: PathBuf = std::env::var("DIST")
        .ok()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("./dist"));
    let data_dir: PathBuf = std::env::var("DATA")
        .ok()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("./reader-data"));
    let token: Option<String> = std::env::var("TOKEN").ok().filter(|t| !t.is_empty());

    if !dist.exists() {
        anyhow::bail!("dist directory not found: {:?}", dist);
    }

    let core = ReaderCore::new(ReaderCoreOptions::new(&data_dir)).await?;
    let state = AppState {
        core: Arc::new(core),
        tasks: TaskRegistry::default(),
        token,
    };

    let addr = format!("{bind}:{port}");
    let app = axum::Router::new()
        .route("/ws", get(ws_handler))
        .route("/asset/:encoded", get(asset_handler))
        .fallback_service(tower_http::services::ServeDir::new(&dist).fallback(
            tower_http::services::ServeFile::new(dist.join("index.html")),
        ))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!("Headless server listening on http://{addr}");
    tracing::info!("WS endpoint: ws://{addr}/ws");
    tracing::info!("Static files: {:?}", dist);

    axum::serve(listener, app).await?;
    Ok(())
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    axum::extract::State(state): axum::extract::State<AppState>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    // Token check
    if let Some(ref expected) = state.token {
        let provided = params.get("token").map(|s| s.as_str()).unwrap_or("");
        if provided != expected {
            return axum::response::Response::builder()
                .status(403)
                .body(axum::body::Body::from("invalid or missing token"))
                .unwrap();
        }
    }
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn asset_handler(
    axum::extract::State(state): axum::extract::State<AppState>,
    Path(encoded): Path<String>,
    Query(params): Query<HashMap<String, String>>,
) -> Response {
    if let Some(ref expected) = state.token {
        let provided = params.get("token").map(|s| s.as_str()).unwrap_or("");
        if provided != expected {
            return StatusCode::FORBIDDEN.into_response();
        }
    }

    let decoded = urlencoding::decode(&encoded)
        .map(|value| value.into_owned())
        .unwrap_or(encoded);
    let path = PathBuf::from(decoded);
    let base = match tokio::fs::canonicalize(state.core.reader_dir()).await {
        Ok(path) => path,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    let target = match tokio::fs::canonicalize(&path).await {
        Ok(path) => path,
        Err(_) => return StatusCode::NOT_FOUND.into_response(),
    };
    if !target.starts_with(&base) {
        return StatusCode::FORBIDDEN.into_response();
    }

    match tokio::fs::read(&target).await {
        Ok(bytes) => {
            let mut response = bytes.into_response();
            if let Some(content_type) = content_type_for_path(&target) {
                response
                    .headers_mut()
                    .insert(header::CONTENT_TYPE, HeaderValue::from_static(content_type));
            }
            response
        }
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn handle_socket(socket: WebSocket, state: AppState) {
    let (mut sink, mut source) = socket.split();
    let out_tx: WsOutgoing = Arc::new(TokioMutex::new(None));
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<String>();
    *out_tx.lock().await = Some(tx);

    // Writer task
    let write_handle = tokio::spawn(async move {
        while let Some(text) = rx.recv().await {
            if sink.send(Message::Text(text.into())).await.is_err() {
                break;
            }
        }
    });

    // Read loop
    while let Some(Ok(msg)) = source.next().await {
        if let Message::Text(text) = msg {
            let state = state.clone();
            let out = out_tx.clone();
            tokio::spawn(async move {
                if let Some(response) = dispatch(&state, &text, &out).await {
                    if let Some(tx) = out.lock().await.as_ref() {
                        let _ = tx.send(response);
                    }
                }
            });
        }
    }

    write_handle.abort();
}

async fn send_ws_event(out: &WsOutgoing, event: &str, payload: Value) {
    let message = json!({
        "type": "event",
        "event": event,
        "payload": payload,
    })
    .to_string();
    if let Some(tx) = out.lock().await.as_ref() {
        let _ = tx.send(message);
    }
}

fn response_err(id: &str, error: String) -> String {
    json!({
        "type": "response",
        "id": id,
        "error": error,
    })
    .to_string()
}

fn cancelled_error() -> String {
    "CANCELLED: 任务已取消".to_string()
}

fn normalize_cancelled_result(
    result: Result<Value, String>,
    token: Option<&Arc<AtomicBool>>,
) -> Result<Value, String> {
    if token
        .map(|token| token.load(Ordering::SeqCst))
        .unwrap_or(false)
    {
        result.map_err(|_| cancelled_error())
    } else {
        result
    }
}

async fn wait_for_cancel(cancelled: Arc<AtomicBool>) {
    while !cancelled.load(Ordering::SeqCst) {
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
}

async fn dispatch(state: &AppState, raw: &str, out: &WsOutgoing) -> Option<String> {
    let msg: WsInbound = serde_json::from_str(raw).ok()?;
    if msg.kind != "invoke" {
        return None;
    }
    let id = msg.id?;
    let cmd = msg.cmd.unwrap_or_default();
    let args = msg.args.unwrap_or_else(|| json!({}));
    let core = &state.core;

    macro_rules! parse_or_response {
        ($ty:ty) => {{
            match serde_json::from_value::<$ty>(args.clone()) {
                Ok(value) => value,
                Err(e) => {
                    let response = json!({
                        "type": "response",
                        "id": id,
                        "error": format!("INVALID_ARGS: {e}"),
                    });
                    return Some(response.to_string());
                }
            }
        }};
    }

    let result: Result<Value, String> = match cmd.as_str() {
        // ── system ──
        "frontend_log" => Ok(Value::Null),
        "get_platform" => Ok(Value::String("headless".to_string())),

        // ── book source ──
        "booksource_list" => core
            .list_sources()
            .await
            .map(|v| serde_json::to_value(v).unwrap_or_default())
            .map_err(|e| e.to_string()),
        "booksource_get_dir" => Ok(Value::String(
            core.js_source_dir().to_string_lossy().to_string(),
        )),
        "booksource_get_dirs" => core
            .source_dirs()
            .await
            .map(|v| serde_json::to_value(v).unwrap_or_default())
            .map_err(|e| e.to_string()),
        "booksource_list_streaming" => {
            let request_id = arg_str(&args, "requestId").unwrap_or("").to_string();
            let force = args.get("force").and_then(Value::as_bool).unwrap_or(false);
            if let Err(err) = core
                .stream_sources(20, force, |items, done, total| {
                    let out = out.clone();
                    let request_id = request_id.clone();
                    async move {
                        send_ws_event(
                            &out,
                            "booksource:batch",
                            json!({
                                "requestId": request_id,
                                "items": items,
                                "done": done,
                                "total": total
                            }),
                        )
                        .await;
                    }
                })
                .await
            {
                return Some(response_err(&id, err.to_string()));
            }
            Ok(Value::Null)
        }
        "booksource_read" => {
            let file_name = arg_str(&args, "fileName").unwrap_or("");
            let source_dir = arg_str(&args, "sourceDir");
            core.read_source(file_name, source_dir)
                .await
                .map(Value::String)
                .map_err(|e| e.to_string())
        }
        "booksource_save" => {
            let file_name = arg_str(&args, "fileName").unwrap_or("");
            let content = arg_str(&args, "content").unwrap_or("");
            let source_dir = arg_str(&args, "sourceDir");
            core.save_js_source(file_name, content, source_dir)
                .await
                .map(|()| Value::Null)
                .map_err(|e| e.to_string())
        }
        "booksource_search" => {
            let keyword = arg_str(&args, "keyword").unwrap_or("");
            let file_name = arg_str(&args, "fileName").unwrap_or("");
            let page = arg_i32(&args, "page").unwrap_or(1);
            let task_id = arg_str(&args, "taskId")
                .filter(|value| !value.is_empty())
                .map(str::to_string);
            let source_dir = arg_str(&args, "sourceDir");
            let token = task_id.as_deref().map(|tid| state.tasks.register(tid));
            if let Some(ref token) = token {
                if token.load(Ordering::SeqCst) {
                    return Some(response_err(&id, cancelled_error()));
                }
            }
            let result = if let Some(token) = token.clone() {
                let search_token = Some(token.clone());
                tokio::select! {
                    result = core.search_with_cancel(file_name, keyword, page, source_dir, search_token) => {
                        result
                            .map(|v| serde_json::to_value(v).unwrap_or_default())
                            .map_err(|e| e.to_string())
                    }
                    _ = wait_for_cancel(token) => Err(cancelled_error()),
                }
            } else {
                core.search_with_cancel(file_name, keyword, page, source_dir, None)
                    .await
                    .map(|v| serde_json::to_value(v).unwrap_or_default())
                    .map_err(|e| e.to_string())
            };
            let result = normalize_cancelled_result(result, token.as_ref());
            if let (Some(task_id), Some(token)) = (task_id.as_deref(), token.as_ref()) {
                state.tasks.remove_if_current(task_id, token);
            }
            result
        }
        "booksource_book_info" => {
            let file_name = arg_str(&args, "fileName").unwrap_or("");
            let book_url = arg_str(&args, "bookUrl").unwrap_or("");
            let source_dir = arg_str(&args, "sourceDir");
            core.book_info(file_name, book_url, source_dir)
                .await
                .map(|v| serde_json::to_value(v).unwrap_or_default())
                .map_err(|e| e.to_string())
        }
        "booksource_chapter_list" => {
            let file_name = arg_str(&args, "fileName").unwrap_or("");
            let book_url = arg_str(&args, "bookUrl").unwrap_or("");
            let task_id = arg_str(&args, "taskId")
                .filter(|value| !value.is_empty())
                .map(str::to_string);
            let source_dir = arg_str(&args, "sourceDir");
            let token = task_id.as_deref().map(|tid| state.tasks.register(tid));
            if let Some(ref token) = token {
                if token.load(Ordering::SeqCst) {
                    return Some(response_err(&id, cancelled_error()));
                }
            }
            let result = if let Some(token) = token.clone() {
                let chapter_token = Some(token.clone());
                tokio::select! {
                    result = core.chapter_list_with_cancel(file_name, book_url, source_dir, chapter_token) => {
                        result
                            .map(|v| serde_json::to_value(v).unwrap_or_default())
                            .map_err(|e| e.to_string())
                    }
                    _ = wait_for_cancel(token) => Err(cancelled_error()),
                }
            } else {
                core.chapter_list_with_cancel(file_name, book_url, source_dir, None)
                    .await
                    .map(|v| serde_json::to_value(v).unwrap_or_default())
                    .map_err(|e| e.to_string())
            };
            let result = normalize_cancelled_result(result, token.as_ref());
            if let (Some(task_id), Some(token)) = (task_id.as_deref(), token.as_ref()) {
                state.tasks.remove_if_current(task_id, token);
            }
            result
        }
        "booksource_chapter_content" => {
            let file_name = arg_str(&args, "fileName").unwrap_or("");
            let chapter_url = arg_str(&args, "chapterUrl").unwrap_or("");
            let task_id = arg_str(&args, "taskId")
                .filter(|value| !value.is_empty())
                .map(str::to_string);
            let source_dir = arg_str(&args, "sourceDir");
            let token = task_id.as_deref().map(|tid| state.tasks.register(tid));
            if let Some(ref token) = token {
                if token.load(Ordering::SeqCst) {
                    return Some(response_err(&id, cancelled_error()));
                }
            }
            let result = if let Some(token) = token.clone() {
                let content_token = Some(token.clone());
                tokio::select! {
                    result = core.chapter_content_with_cancel(file_name, chapter_url, source_dir, content_token) => {
                        result
                            .map(|v| serde_json::to_value(v).unwrap_or_default())
                            .map_err(|e| e.to_string())
                    }
                    _ = wait_for_cancel(token) => Err(cancelled_error()),
                }
            } else {
                core.chapter_content_with_cancel(file_name, chapter_url, source_dir, None)
                    .await
                    .map(|v| serde_json::to_value(v).unwrap_or_default())
                    .map_err(|e| e.to_string())
            };
            let result = normalize_cancelled_result(result, token.as_ref());
            if let (Some(task_id), Some(token)) = (task_id.as_deref(), token.as_ref()) {
                state.tasks.remove_if_current(task_id, token);
            }
            result
        }
        "booksource_import_legacy_json_text" => {
            let content = arg_str(&args, "content").unwrap_or("");
            let smart_explore_sub_categories =
                arg_bool(&args, "smartExploreSubCategories").unwrap_or(false);
            core.import_legacy_json_text(content, smart_explore_sub_categories)
                .await
                .map(|v| serde_json::to_value(v).unwrap_or_default())
                .map_err(|e| e.to_string())
        }
        "booksource_delete" => {
            let file_name = arg_str(&args, "fileName").unwrap_or("");
            core.delete_source(file_name, None)
                .await
                .map(|()| Value::Null)
                .map_err(|e| e.to_string())
        }
        "booksource_toggle" => {
            let file_name = arg_str(&args, "fileName").unwrap_or("");
            let enabled = arg_bool(&args, "enabled").unwrap_or(true);
            let source_dir = arg_str(&args, "sourceDir");
            core.toggle_source(file_name, enabled, source_dir)
                .await
                .map(|()| Value::Null)
                .map_err(|e| e.to_string())
        }
        "booksource_explore" => {
            let file_name = arg_str(&args, "fileName").unwrap_or("");
            let category = arg_str(&args, "category").unwrap_or("");
            let page = arg_i32(&args, "page").unwrap_or(1);
            let source_dir = arg_str(&args, "sourceDir");
            core.explore(file_name, page, category, source_dir)
                .await
                .map(|v| serde_json::to_value(v).unwrap_or_default())
                .map_err(|e| e.to_string())
        }
        "booksource_run_tests" => {
            let file_name = arg_str(&args, "fileName").unwrap_or("");
            core.run_source_tests(file_name, None, None, Some(30))
                .await
                .map(|v| serde_json::to_value(v).unwrap_or_default())
                .map_err(|e| e.to_string())
        }
        "booksource_cancel" => {
            let task_id = arg_str(&args, "taskId").unwrap_or("");
            if state.tasks.cancel(task_id) {
                Ok(Value::Null)
            } else {
                Err(format!("TASK_NOT_FOUND: 任务 {task_id} 不存在或已完成"))
            }
        }

        // ── shelf ──
        "bookshelf_list" => core
            .shelf_list()
            .await
            .map(|v| serde_json::to_value(v).unwrap_or_default())
            .map_err(|e| e.to_string()),
        "bookshelf_add" => {
            let payload = parse_or_response!(BookshelfAddArgs);
            core.shelf_add(payload.book, &payload.file_name, &payload.source_name)
                .await
                .map(|v| serde_json::to_value(v).unwrap_or_default())
                .map_err(|e| e.to_string())
        }
        "bookshelf_remove" => {
            let id = arg_str(&args, "id").unwrap_or("");
            core.shelf_remove(&id)
                .await
                .map(|()| Value::Null)
                .map_err(|e| e.to_string())
        }
        "bookshelf_get" => {
            let id = arg_str(&args, "id").unwrap_or("");
            core.shelf_get(&id)
                .await
                .map(|v| serde_json::to_value(v).unwrap_or_default())
                .map_err(|e| e.to_string())
        }
        "bookshelf_get_chapters" => {
            let id = arg_str(&args, "id").unwrap_or("");
            core.shelf_get_chapters(&id)
                .await
                .map(|v| serde_json::to_value(v).unwrap_or_default())
                .map_err(|e| e.to_string())
        }
        "bookshelf_update_progress" => {
            let payload = parse_or_response!(UpdateProgressArgs);
            core.shelf_update_progress(
                &payload.id,
                payload.chapter_index,
                &payload.chapter_url,
                payload.page_index,
                payload.scroll_ratio,
                payload.playback_time,
                payload.reader_settings,
            )
            .await
            .map(|()| Value::Null)
            .map_err(|e| e.to_string())
        }
        "bookshelf_set_private" => {
            let payload = parse_or_response!(SetPrivateArgs);
            core.shelf_set_private(&payload.id, payload.is_private)
                .await
                .map(|()| Value::Null)
                .map_err(|e| e.to_string())
        }
        "bookshelf_save_chapters" => {
            let payload = parse_or_response!(SaveChaptersArgs);
            core.shelf_save_chapters(&payload.id, payload.chapters)
                .await
                .map(|()| Value::Null)
                .map_err(|e| e.to_string())
        }
        "bookshelf_update_book" => {
            let payload = parse_or_response!(UpdateBookArgs);
            core.shelf_update_book(payload.book, payload.chapters)
                .await
                .map(|v| serde_json::to_value(v).unwrap_or_default())
                .map_err(|e| e.to_string())
        }
        "bookshelf_save_content" => {
            let payload = parse_or_response!(ContentArgs);
            core.shelf_save_content(&payload.id, payload.chapter_index, &payload.content)
                .await
                .map(|()| Value::Null)
                .map_err(|e| e.to_string())
        }
        "bookshelf_get_content" => {
            let payload = parse_or_response!(ContentKeyArgs);
            core.shelf_get_content(&payload.id, payload.chapter_index)
                .await
                .map(|v| serde_json::to_value(v).unwrap_or_default())
                .map_err(|e| e.to_string())
        }
        "bookshelf_delete_content" => {
            let payload = parse_or_response!(ContentKeyArgs);
            core.shelf_delete_content(&payload.id, payload.chapter_index)
                .await
                .map(|()| Value::Null)
                .map_err(|e| e.to_string())
        }
        "bookshelf_get_cached_indices" => {
            let id = arg_str(&args, "id").unwrap_or("");
            core.shelf_cached_indices(&id)
                .await
                .map(|v| serde_json::to_value(v).unwrap_or_default())
                .map_err(|e| e.to_string())
        }
        "bookshelf_get_episode_progress" => {
            let id = arg_str(&args, "id").unwrap_or("");
            core.shelf_get_episode_progress(&id)
                .await
                .map(|v| serde_json::to_value(v).unwrap_or_default())
                .map_err(|e| e.to_string())
        }
        "bookshelf_save_episode_progress" => {
            let payload = parse_or_response!(EpisodeProgressArgs);
            core.shelf_save_episode_progress(
                &payload.id,
                &payload.chapter_url,
                payload.time,
                payload.duration,
            )
            .await
            .map(|()| Value::Null)
            .map_err(|e| e.to_string())
        }
        "bookshelf_restore_source_switch" => {
            let id = arg_str(&args, "id").unwrap_or("");
            core.shelf_restore_source_switch(&id)
                .await
                .map(|v| serde_json::to_value(v).unwrap_or_default())
                .map_err(|e| e.to_string())
        }

        // ── config ──
        "config_read" => {
            let scope = arg_str(&args, "scope").unwrap_or("");
            let key = arg_str(&args, "key").unwrap_or("");
            core.config_read(&scope, &key)
                .await
                .map(|v| Value::String(v))
                .map_err(|e| e.to_string())
        }
        "config_write" => {
            let scope = arg_str(&args, "scope").unwrap_or("");
            let key = arg_str(&args, "key").unwrap_or("");
            let value = arg_str(&args, "value").unwrap_or("");
            core.config_write(&scope, &key, &value)
                .await
                .map(|()| Value::Null)
                .map_err(|e| e.to_string())
        }
        "config_read_json" => {
            let scope = arg_str(&args, "scope").unwrap_or("");
            let key = arg_str(&args, "key").unwrap_or("");
            core.config_read_json(&scope, &key)
                .await
                .map(|value| value.unwrap_or(Value::Null))
                .map_err(|e| e.to_string())
        }
        "config_write_json" => {
            let scope = arg_str(&args, "scope").unwrap_or("");
            let key = arg_str(&args, "key").unwrap_or("");
            let value = args.get("value").cloned().unwrap_or(Value::Null);
            core.config_write_json(&scope, &key, &value)
                .await
                .map(|()| Value::Null)
                .map_err(|e| e.to_string())
        }
        "config_delete_key" => {
            let scope = arg_str(&args, "scope").unwrap_or("");
            let key = arg_str(&args, "key").unwrap_or("");
            core.config_delete_key(&scope, &key)
                .await
                .map(|()| Value::Null)
                .map_err(|e| e.to_string())
        }
        "config_read_all" => {
            let scope = arg_str(&args, "scope").unwrap_or("");
            core.config_read_all(&scope)
                .await
                .map(Value::String)
                .map_err(|e| e.to_string())
        }
        "config_clear" => {
            let scope = arg_str(&args, "scope").unwrap_or("");
            core.config_clear(&scope)
                .await
                .map(|()| Value::Null)
                .map_err(|e| e.to_string())
        }
        "config_list_scopes" => core
            .config_list_scopes()
            .await
            .map(|v| serde_json::to_value(v).unwrap_or_default())
            .map_err(|e| e.to_string()),
        "frontend_storage_list" => {
            let namespace = arg_str(&args, "namespace").unwrap_or("");
            core.frontend_storage_list(&namespace)
                .await
                .map(|v| serde_json::to_value(v).unwrap_or_default())
                .map_err(|e| e.to_string())
        }
        "frontend_storage_set" => {
            let namespace = arg_str(&args, "namespace").unwrap_or("");
            let key = arg_str(&args, "key").unwrap_or("");
            let value = arg_str(&args, "value").unwrap_or("");
            core.frontend_storage_set(&namespace, &key, &value)
                .await
                .map(|()| Value::Null)
                .map_err(|e| e.to_string())
        }
        "frontend_storage_remove" => {
            let namespace = arg_str(&args, "namespace").unwrap_or("");
            let key = arg_str(&args, "key").unwrap_or("");
            core.frontend_storage_remove(&namespace, &key)
                .await
                .map(|()| Value::Null)
                .map_err(|e| e.to_string())
        }
        "frontend_storage_list_namespaces" => core
            .frontend_storage_list_namespaces()
            .await
            .map(|v| serde_json::to_value(v).unwrap_or_default())
            .map_err(|e| e.to_string()),
        "app_config_get_all" => core.app_config_get_all().await.map_err(|e| e.to_string()),
        "app_config_set" => {
            let key = arg_str(&args, "key").unwrap_or("");
            let value = args.get("value").cloned().unwrap_or(Value::Null);
            core.app_config_set(&key, &value)
                .await
                .map(|()| Value::Null)
                .map_err(|e| e.to_string())
        }
        "app_config_reset" => {
            let key = arg_str(&args, "key").unwrap_or("");
            core.app_config_reset(&key)
                .await
                .map(|()| Value::Null)
                .map_err(|e| e.to_string())
        }
        "storage_debug_dump" => core.debug_dump().await.map_err(|e| e.to_string()),
        "cover_cache_size" => core
            .cover_cache_size()
            .await
            .map(|value| json!(value))
            .map_err(|e| e.to_string()),
        "cover_cache_clear" => core
            .clear_cover_cache()
            .await
            .map(|value| json!(value))
            .map_err(|e| e.to_string()),
        "cover_resolve_cache" => {
            let payload = parse_or_response!(CoverResolveArgs);
            core.resolve_cover_cache(
                &payload.request.url,
                payload.request.referer.as_deref(),
                payload.request.headers.as_ref(),
            )
            .await
            .map(|local_path| {
                json!({
                    "localPath": local_path,
                    "localRef": format!("local://{local_path}"),
                })
            })
            .map_err(|e| e.to_string())
        }

        // ── capabilities (transport-agnostic) ──
        "capabilities_get" => Ok(json!({
            "syncWebdav": unsupported_capability("WebDAV sync is not exposed by legado-headless yet.", [
                "sync_set_credentials",
                "sync_get_credentials",
                "sync_clear_credentials",
                "sync_get_status",
                "sync_now",
                "sync_test_connection",
                "sync_list_conflicts",
                "sync_resolve_conflict",
                "sync_notify_lifecycle",
                "sync_client_state_set",
                "sync_report_reader_session",
                "sync_v2_sync_reading_progress",
            ]),
            "sync": unsupported_capability("Baidu Netdisk and FTP sync providers are not implemented in this build.", [
                "sync_baidu_start_auth",
                "sync_baidu_poll_token",
                "sync_baidu_token_status",
                "sync_baidu_revoke_auth",
            ]),
            "tts": unsupported_capability("Native TTS backend is not available in headless mode.", [
                "tts_get_voices",
                "tts_is_initialized",
                "tts_is_speaking",
                "tts_speak",
                "tts_stop",
                "tts_preview_voice",
            ]),
            "videoProxy": unsupported_capability("Local video proxy is not available in headless mode.", [
                "start_video_proxy",
                "stop_video_proxy",
            ]),
            "browserProbe": unsupported_capability("Headless browser probe is not implemented in this build.", [
                "browser_probe_create",
                "browser_probe_close",
                "browser_probe_close_all",
                "browser_probe_hide",
                "browser_probe_show",
                "browser_probe_navigate",
                "browser_probe_eval",
                "browser_probe_run",
                "browser_probe_get_cookies",
                "browser_probe_set_cookie",
                "browser_probe_clear_data",
                "browser_probe_set_user_agent",
            ]),
            "comicCache": unsupported_capability("Comic page cache is not implemented in this build.", [
                "comic_cache_clear",
                "comic_cache_clear_chapter",
                "comic_cache_size",
                "comic_download_images",
                "comic_get_cached_page",
                "comic_get_page_sizes",
            ]),
            "coverCache": supported_capability("Cover disk cache is implemented for HTTP/HTTPS book cover images.", [
                "cover_cache_clear",
                "cover_cache_size",
                "cover_resolve_cache",
            ]),
            "repository": unsupported_capability("Source repository commands are not exposed by legado-headless yet.", [
                "repository_fetch",
                "repository_install",
                "repository_preview_source",
                "repository_check_source_sync",
                "booksource_check_update",
                "booksource_apply_update",
            ]),
            "unlock": unsupported_capability("Secure-mode unlock challenges are not implemented in this build.", [
                "issue_full_mode_challenge",
                "verify_full_mode_challenge",
                "issue_scoped_unlock_challenge",
                "verify_scoped_unlock_challenge",
            ]),
            "aiProxy": unsupported_capability("AI HTTP proxy is not implemented in this build.", [
                "ai_http_proxy_url",
            ]),
            "pluginHttp": unsupported_capability("Frontend plugin HTTP bridge is not implemented in this build.", [
                "frontend_plugin_http_request",
            ]),
            "exploreCache": unsupported_capability("Explore result cache is not implemented in this build.", [
                "explore_clear_cache",
            ]),
        })),

        // ── explicitly blocked ──
        "js_eval" => Err("security_blocked: js_eval is not available via WebSocket".to_string()),

        _ => Err(format!("NOT_ROUTED: {cmd}")),
    };

    let response = match result {
        Ok(data) => json!({ "type": "response", "id": id, "data": data }),
        Err(error) => json!({ "type": "response", "id": id, "error": error }),
    };
    Some(response.to_string())
}

fn arg_str<'a>(args: &'a Value, key: &str) -> Option<&'a str> {
    args.get(key).and_then(|v| v.as_str())
}

fn arg_i32(args: &Value, key: &str) -> Option<i32> {
    args.get(key)
        .and_then(|v| v.as_i64())
        .and_then(|v| i32::try_from(v).ok())
}

fn arg_bool(args: &Value, key: &str) -> Option<bool> {
    args.get(key).and_then(|v| v.as_bool())
}

fn content_type_for_path(path: &std::path::Path) -> Option<&'static str> {
    match path
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase())
        .as_deref()
    {
        Some("jpg") | Some("jpeg") => Some("image/jpeg"),
        Some("png") => Some("image/png"),
        Some("webp") => Some("image/webp"),
        Some("gif") => Some("image/gif"),
        Some("bmp") => Some("image/bmp"),
        Some("avif") => Some("image/avif"),
        Some("svg") => Some("image/svg+xml"),
        _ => None,
    }
}

fn unsupported_capability<const N: usize>(reason: &str, commands: [&str; N]) -> Value {
    json!({
        "supported": false,
        "reason": reason,
        "commands": commands.as_slice(),
    })
}

fn supported_capability<const N: usize>(reason: &str, commands: [&str; N]) -> Value {
    json!({
        "supported": true,
        "reason": reason,
        "commands": commands.as_slice(),
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct BookshelfAddArgs {
    book: AddBookPayload,
    file_name: String,
    source_name: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateProgressArgs {
    id: String,
    chapter_index: i32,
    chapter_url: String,
    page_index: Option<i32>,
    scroll_ratio: Option<f64>,
    playback_time: Option<f64>,
    reader_settings: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SetPrivateArgs {
    id: String,
    is_private: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SaveChaptersArgs {
    id: String,
    chapters: Vec<CachedChapter>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateBookArgs {
    book: UpdateShelfBookPayload,
    chapters: Option<Vec<CachedChapter>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ContentArgs {
    id: String,
    chapter_index: i32,
    content: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ContentKeyArgs {
    id: String,
    chapter_index: i32,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct EpisodeProgressArgs {
    id: String,
    chapter_url: String,
    time: f64,
    duration: f64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CoverResolveArgs {
    request: CoverResolveRequest,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CoverResolveRequest {
    url: String,
    referer: Option<String>,
    headers: Option<HashMap<String, String>>,
}

fn parse_env_or_arg(env_name: &str, arg_name: &str, default: u16) -> u16 {
    std::env::var(env_name)
        .ok()
        .and_then(|v| v.parse().ok())
        .or_else(|| {
            std::env::args()
                .collect::<Vec<_>>()
                .windows(2)
                .find(|w| w[0] == arg_name)
                .and_then(|w| w[1].parse().ok())
        })
        .unwrap_or(default)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::time::{SystemTime, UNIX_EPOCH};

    async fn test_state() -> (AppState, PathBuf) {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "legado-headless-formb-test-{}-{stamp}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let core = ReaderCore::new(ReaderCoreOptions::new(&dir)).await.unwrap();
        (
            AppState {
                core: Arc::new(core),
                tasks: TaskRegistry::default(),
                token: None,
            },
            dir,
        )
    }

    async fn invoke(state: &AppState, cmd: &str, args: Value) -> Value {
        let raw = json!({
            "type": "invoke",
            "id": format!("test-{cmd}"),
            "cmd": cmd,
            "args": args,
        })
        .to_string();
        let out: WsOutgoing = Arc::new(TokioMutex::new(None));
        let response = dispatch(state, &raw, &out).await.expect("response");
        let value: Value = serde_json::from_str(&response).expect("response json");
        if let Some(error) = value.get("error") {
            panic!("{cmd} failed: {error}");
        }
        value.get("data").cloned().unwrap_or(Value::Null)
    }

    async fn invoke_response(state: &AppState, cmd: &str, args: Value) -> Value {
        let raw = json!({
            "type": "invoke",
            "id": format!("test-{cmd}"),
            "cmd": cmd,
            "args": args,
        })
        .to_string();
        let out: WsOutgoing = Arc::new(TokioMutex::new(None));
        let response = dispatch(state, &raw, &out).await.expect("response");
        serde_json::from_str(&response).expect("response json")
    }

    struct JsEngineTimeoutRestore;

    impl Drop for JsEngineTimeoutRestore {
        fn drop(&mut self) {
            reader_core::parser::js::set_js_engine_timeout_secs(0);
        }
    }

    fn set_js_engine_timeout_for_test(secs: u64) -> JsEngineTimeoutRestore {
        reader_core::parser::js::set_js_engine_timeout_secs(secs);
        JsEngineTimeoutRestore
    }

    #[tokio::test]
    async fn booksource_cancel_interrupts_headless_search_task() {
        let _timeout = set_js_engine_timeout_for_test(3);
        let (state, dir) = test_state().await;
        let file_name = "runaway-headless-search.js";
        state
            .core
            .save_js_source(
                file_name,
                r#"// @name        Runaway Headless Search
// @url         https://example.invalid
// @enabled     true

async function search() {
  while (true) {}
}
"#,
                None,
            )
            .await
            .unwrap();

        let raw = json!({
            "type": "invoke",
            "id": "search-cancel-target",
            "cmd": "booksource_search",
            "args": {
                "fileName": file_name,
                "keyword": "anything",
                "page": 1,
                "taskId": "headless-search-cancel",
                "sourceDir": null
            },
        })
        .to_string();
        let search_state = state.clone();
        let search_handle = tokio::spawn(async move {
            let out: WsOutgoing = Arc::new(TokioMutex::new(None));
            let response = dispatch(&search_state, &raw, &out).await.expect("response");
            serde_json::from_str::<Value>(&response).expect("response json")
        });

        let mut cancel_response = None;
        for _ in 0..20 {
            let response = invoke_response(
                &state,
                "booksource_cancel",
                json!({"taskId": "headless-search-cancel"}),
            )
            .await;
            if response.get("error").is_none() {
                cancel_response = Some(response);
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(25)).await;
        }
        assert!(
            cancel_response.is_some(),
            "booksource_cancel should reach registered headless task"
        );

        let search_response =
            tokio::time::timeout(std::time::Duration::from_secs(2), search_handle)
                .await
                .expect("cancelled search should complete before JS engine timeout")
                .expect("search task should not panic");
        let error = search_response
            .get("error")
            .and_then(Value::as_str)
            .unwrap_or("");
        assert!(
            error.contains("CANCELLED"),
            "search should return CANCELLED after cancel, got {search_response}"
        );

        let missing = invoke_response(
            &state,
            "booksource_cancel",
            json!({"taskId": "headless-search-cancel"}),
        )
        .await;
        let error = missing.get("error").and_then(Value::as_str).unwrap_or("");
        assert!(
            error.contains("TASK_NOT_FOUND"),
            "completed task id should be removed, got {missing}"
        );

        let _ = std::fs::remove_dir_all(dir);
    }

    #[tokio::test]
    async fn formb_accept_headless_dispatch_chain() {
        let (state, dir) = test_state().await;
        let source = r#"// @name        FORMB Fixture
// @url         fixture://formb
// @enabled     true

async function search(key, page) {
  return [{
    name: '形态B验收书',
    author: 'Codex',
    bookUrl: 'fixture://formb/book/1',
    intro: '用于浏览器形态B闭环验收',
    kind: '验收',
    coverUrl: '',
    tocUrl: 'fixture://formb/book/1/toc'
  }];
}

async function bookInfo(bookUrl) {
  return {
    name: '形态B验收书',
    author: 'Codex',
    bookUrl,
    intro: '用于浏览器形态B闭环验收',
    kind: '验收',
    coverUrl: '',
    tocUrl: 'fixture://formb/book/1/toc',
    lastChapter: '第二章 继续前进'
  };
}

async function chapterList(tocUrl) {
  return [
    { name: '第一章 浏览器闭环', url: 'fixture://formb/chapter/1' },
    { name: '第二章 继续前进', url: 'fixture://formb/chapter/2' }
  ];
}

async function chapterContent(chapterUrl) {
  if (chapterUrl.endsWith('/2')) return '第二章正文：进度保存后仍可读取。';
  return '第一章正文：纯浏览器前端通过 WebSocket 调用 headless 后端读取。';
}
"#;
        let file_name = "formb-fixture.js";

        invoke(
            &state,
            "booksource_save",
            json!({"fileName": file_name, "content": source, "sourceDir": null}),
        )
        .await;
        let sources = invoke(&state, "booksource_list", json!({})).await;
        assert_eq!(sources.as_array().unwrap().len(), 1);

        let search = invoke(
            &state,
            "booksource_search",
            json!({"fileName": file_name, "keyword": "形态B", "page": 1, "sourceDir": null}),
        )
        .await;
        let book = search.as_array().unwrap().first().unwrap();
        assert_eq!(book["name"], "形态B验收书");

        let detail = invoke(
            &state,
            "booksource_book_info",
            json!({"fileName": file_name, "bookUrl": book["bookUrl"], "sourceDir": null}),
        )
        .await;
        assert_eq!(detail["tocUrl"], "fixture://formb/book/1/toc");

        let shelf = invoke(
            &state,
            "bookshelf_add",
            json!({
                "book": {
                    "name": detail["name"],
                    "author": detail["author"],
                    "coverUrl": detail["coverUrl"],
                    "intro": detail["intro"],
                    "kind": detail["kind"],
                    "groupId": null,
                    "bookUrl": detail["bookUrl"],
                    "sourceDir": null,
                    "lastChapter": detail["lastChapter"],
                    "sourceType": "novel"
                },
                "fileName": file_name,
                "sourceName": "FORMB Fixture"
            }),
        )
        .await;
        let shelf_id = shelf["id"].as_str().unwrap();

        let raw_chapters = invoke(
            &state,
            "booksource_chapter_list",
            json!({"fileName": file_name, "bookUrl": detail["tocUrl"], "taskId": null, "sourceDir": null}),
        )
        .await;
        let cached_chapters: Vec<Value> = raw_chapters
            .as_array()
            .unwrap()
            .iter()
            .enumerate()
            .map(|(index, ch)| {
                json!({
                    "index": index,
                    "name": ch["name"],
                    "url": ch["url"],
                    "group": ch.get("group").cloned().unwrap_or(Value::Null),
                    "vip": ch.get("vip").or_else(|| ch.get("isVip")).cloned().unwrap_or(Value::Null),
                    "price": ch.get("price").cloned().unwrap_or(Value::Null),
                    "currency": ch.get("currency").cloned().unwrap_or(Value::Null),
                })
            })
            .collect();
        invoke(
            &state,
            "bookshelf_save_chapters",
            json!({"id": shelf_id, "chapters": cached_chapters}),
        )
        .await;
        let saved = invoke(&state, "bookshelf_get_chapters", json!({"id": shelf_id})).await;
        assert_eq!(saved.as_array().unwrap().len(), 2);

        let content = invoke(
            &state,
            "booksource_chapter_content",
            json!({"fileName": file_name, "chapterUrl": "fixture://formb/chapter/1", "sourceDir": null}),
        )
        .await;
        assert!(content.as_str().unwrap().contains("第一章正文"));
        invoke(
            &state,
            "bookshelf_save_content",
            json!({"id": shelf_id, "chapterIndex": 0, "content": content}),
        )
        .await;
        let cached = invoke(
            &state,
            "bookshelf_get_content",
            json!({"id": shelf_id, "chapterIndex": 0}),
        )
        .await;
        assert!(cached.as_str().unwrap().contains("WebSocket"));

        invoke(
            &state,
            "bookshelf_update_progress",
            json!({
                "id": shelf_id,
                "chapterIndex": 1,
                "chapterUrl": "fixture://formb/chapter/2",
                "pageIndex": 3,
                "scrollRatio": 0.42,
                "playbackTime": null,
                "readerSettings": "{\"mode\":\"formb-test\"}"
            }),
        )
        .await;
        let after = invoke(&state, "bookshelf_get", json!({"id": shelf_id})).await;
        assert_eq!(after["readChapterIndex"], 1);
        assert_eq!(after["readChapterUrl"], "fixture://formb/chapter/2");
        assert_eq!(after["readPageIndex"], 3);
        assert_eq!(after["readScrollRatio"], 0.42);

        let _ = std::fs::remove_dir_all(dir);
    }
}
