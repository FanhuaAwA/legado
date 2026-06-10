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
use axum::extract::WebSocketUpgrade;
use axum::response::IntoResponse;
use axum::routing::get;
use futures::{SinkExt, StreamExt};
use reader_core::{AddBookPayload, ReaderCore, ReaderCoreOptions};
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Clone)]
struct AppState {
    core: Arc<ReaderCore>,
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
        token,
    };

    let addr = format!("{bind}:{port}");
    let app = axum::Router::new()
        .route("/ws", get(ws_handler))
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

async fn handle_socket(socket: WebSocket, state: AppState) {
    let (mut sink, mut source) = socket.split();
    let out_tx: Arc<Mutex<Option<tokio::sync::mpsc::UnboundedSender<String>>>> =
        Arc::new(Mutex::new(None));
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
                if let Some(response) = dispatch(&state, &text).await {
                    if let Some(tx) = out.lock().await.as_ref() {
                        let _ = tx.send(response);
                    }
                }
            });
        }
    }

    write_handle.abort();
}

async fn dispatch(state: &AppState, raw: &str) -> Option<String> {
    let msg: WsInbound = serde_json::from_str(raw).ok()?;
    if msg.kind != "invoke" {
        return None;
    }
    let id = msg.id?;
    let cmd = msg.cmd.unwrap_or_default();
    let args = msg.args.unwrap_or_else(|| json!({}));
    let core = &state.core;

    let result: Result<Value, String> = match cmd.as_str() {
        // ── book source ──
        "booksource_list" => core
            .list_sources()
            .await
            .map(|v| serde_json::to_value(v).unwrap_or_default())
            .map_err(|e| e.to_string()),
        "booksource_search" => {
            let keyword = arg_str(&args, "keyword").unwrap_or("");
            let file_name = arg_str(&args, "fileName").unwrap_or("");
            core.search(file_name, keyword, 1, None)
                .await
                .map(|v| serde_json::to_value(v).unwrap_or_default())
                .map_err(|e| e.to_string())
        }
        "booksource_book_info" => {
            let file_name = arg_str(&args, "fileName").unwrap_or("");
            let book_url = arg_str(&args, "bookUrl").unwrap_or("");
            core.book_info(file_name, book_url, None)
                .await
                .map(|v| serde_json::to_value(v).unwrap_or_default())
                .map_err(|e| e.to_string())
        }
        "booksource_chapter_list" => {
            let file_name = arg_str(&args, "fileName").unwrap_or("");
            let book_url = arg_str(&args, "bookUrl").unwrap_or("");
            core.chapter_list(file_name, book_url, None)
                .await
                .map(|v| serde_json::to_value(v).unwrap_or_default())
                .map_err(|e| e.to_string())
        }
        "booksource_chapter_content" => {
            let file_name = arg_str(&args, "fileName").unwrap_or("");
            let chapter_url = arg_str(&args, "chapterUrl").unwrap_or("");
            core.chapter_content(file_name, chapter_url, None)
                .await
                .map(|v| serde_json::to_value(v).unwrap_or_default())
                .map_err(|e| e.to_string())
        }
        "booksource_import_legacy_json_text" => {
            let content = arg_str(&args, "content").unwrap_or("");
            core.import_legacy_json_text(content, false)
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
            core.toggle_source(file_name, enabled, None)
                .await
                .map(|()| Value::Null)
                .map_err(|e| e.to_string())
        }
        "booksource_explore" => {
            let file_name = arg_str(&args, "fileName").unwrap_or("");
            let category = arg_str(&args, "category").unwrap_or("");
            core.explore(file_name, 1, category, None)
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

        // ── shelf ──
        "bookshelf_list" => core
            .shelf_list()
            .await
            .map(|v| serde_json::to_value(v).unwrap_or_default())
            .map_err(|e| e.to_string()),
        "bookshelf_add" => {
            let payload: AddPayload = serde_json::from_value(args)
                .map_err(|e| e.to_string())
                .ok()?;
            core.shelf_add(
                AddBookPayload {
                    name: String::new(),
                    author: None,
                    cover_url: None,
                    intro: None,
                    kind: None,
                    group_id: None,
                    book_url: payload.book_url.clone(),
                    source_dir: payload.source_dir.clone(),
                    last_chapter: None,
                    source_type: None,
                },
                &payload.file_name,
                &payload.book_url,
            )
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
            let id = arg_str(&args, "id").unwrap_or("");
            let chapter_url = arg_str(&args, "chapterUrl").unwrap_or("");
            let time = arg_f64(&args, "time").unwrap_or(0.0);
            let duration = arg_f64(&args, "duration").unwrap_or(0.0);
            core.shelf_save_episode_progress(&id, &chapter_url, time, duration)
                .await
                .map(|()| Value::Null)
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

        // ── capabilities (transport-agnostic) ──
        "capabilities_get" => Ok(json!({
            "booksource": {"supported": true},
            "bookshelf": {"supported": true},
            "config": {"supported": true},
            "backup": {"supported": true},
            "reader": {"supported": true},
            "explore": {"supported": true},
            "sync": {"supported": false, "reason": "unsupported_in_headless"},
            "tts": {"supported": false, "reason": "unsupported_in_headless"},
            "video": {"supported": false, "reason": "unsupported_in_headless"},
            "comic": {"supported": false, "reason": "unsupported_in_headless"},
            "cover": {"supported": false, "reason": "unsupported_in_headless"},
            "update": {"supported": false, "reason": "unsupported_in_headless"},
            "browser_probe": {"supported": false, "reason": "unsupported_in_headless"},
            "plugin_http": {"supported": false, "reason": "unsupported_in_headless"},
        })),

        "get_platform" => Ok(Value::String("headless".to_string())),

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

fn arg_f64(args: &Value, key: &str) -> Option<f64> {
    args.get(key).and_then(|v| v.as_f64())
}

fn arg_bool(args: &Value, key: &str) -> Option<bool> {
    args.get(key).and_then(|v| v.as_bool())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AddPayload {
    book_url: String,
    file_name: String,
    source_dir: Option<String>,
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
