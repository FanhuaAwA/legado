//! 应用内 WebSocket 命令服务端（R-P2-008 阶段 2 试点）
//!
//! 协议契约与 `src/composables/useTransport.ts` 头部注释一致，
//! 权威说明见 docs/frontend-backend-separation.md 第 3 节：
//!
//! ```json
//! 客户端 → 服务器：{ "type": "invoke", "id": "uuid", "cmd": "...", "args": {} }
//! 服务器 → 客户端：{ "type": "response", "id": "uuid", "data": ... }   // 失败带 "error"
//! 服务器 → 客户端：{ "type": "event", "event": "...", "payload": ... }
//! ```
//!
//! 安全边界（试点阶段）：
//! - 仅绑定 127.0.0.1，LAN/公网暴露属阶段 3（鉴权 token + 显式开关），当前不可配置。
//! - 仅接受路径 `/ws` 的升级请求。
//! - 命令白名单由 commands/router.rs 的 match 承担；`js_eval` 永久阻断。

use futures::{SinkExt, StreamExt};
use serde::Deserialize;
use serde_json::{json, Value};
use tauri::Listener;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{broadcast, mpsc};
use tokio_tungstenite::tungstenite::handshake::server::{ErrorResponse, Request, Response};
use tokio_tungstenite::tungstenite::Message;

use crate::commands::router;

const BIND_ADDR: &str = "127.0.0.1:7688";

/// 向 WS 客户端转发的后端事件清单。
/// Tauri v2 没有「监听全部事件」的 API，新增后端事件名时必须同步追加，
/// 否则 WS 客户端收不到（事件名是跨进程契约，见纪律文档第 4 节第 5 条）。
const FORWARDED_EVENTS: &[&str] = &["rust:log", "app_config:changed", "script:dialog:result"];

#[derive(Deserialize)]
struct InboundMessage {
    #[serde(rename = "type")]
    kind: String,
    id: Option<String>,
    cmd: Option<String>,
    args: Option<Value>,
}

/// 启动 WS 命令服务端（绑定失败只告警不致命，桌面 IPC 路径不受影响）
pub fn start(app: tauri::AppHandle) {
    let (event_tx, _) = broadcast::channel::<(String, Value)>(256);
    for name in FORWARDED_EVENTS {
        let tx = event_tx.clone();
        let event_name = (*name).to_string();
        app.listen_any(*name, move |event| {
            let payload: Value = serde_json::from_str(event.payload()).unwrap_or(Value::Null);
            let _ = tx.send((event_name.clone(), payload));
        });
    }

    tauri::async_runtime::spawn(async move {
        let listener = match TcpListener::bind(BIND_ADDR).await {
            Ok(listener) => listener,
            Err(e) => {
                tracing::warn!("WS 命令服务端绑定 {BIND_ADDR} 失败（端口被占用？）: {e}");
                return;
            }
        };
        tracing::info!("WS 命令服务端已监听 ws://{BIND_ADDR}/ws");
        loop {
            match listener.accept().await {
                Ok((stream, peer)) => {
                    tracing::debug!("WS 连接接入: {peer}");
                    let app = app.clone();
                    let event_rx = event_tx.subscribe();
                    tauri::async_runtime::spawn(async move {
                        handle_connection(app, stream, event_rx).await;
                        tracing::debug!("WS 连接关闭: {peer}");
                    });
                }
                Err(e) => {
                    tracing::warn!("WS accept 失败: {e}");
                }
            }
        }
    });
}

async fn handle_connection(
    app: tauri::AppHandle,
    stream: TcpStream,
    mut event_rx: broadcast::Receiver<(String, Value)>,
) {
    let path_check = |req: &Request, resp: Response| -> Result<Response, ErrorResponse> {
        if req.uri().path() == "/ws" {
            Ok(resp)
        } else {
            let mut not_found = ErrorResponse::new(Some("not found".to_string()));
            *not_found.status_mut() = tokio_tungstenite::tungstenite::http::StatusCode::NOT_FOUND;
            Err(not_found)
        }
    };
    let ws = match tokio_tungstenite::accept_hdr_async(stream, path_check).await {
        Ok(ws) => ws,
        Err(e) => {
            tracing::debug!("WS 握手失败: {e}");
            return;
        }
    };
    let (mut sink, mut source) = ws.split();

    // 出站统一走 mpsc，命令响应与事件推送共用一个写通道，避免并发写 sink
    let (out_tx, mut out_rx) = mpsc::unbounded_channel::<String>();
    let writer = tauri::async_runtime::spawn(async move {
        while let Some(text) = out_rx.recv().await {
            if sink.send(Message::Text(text)).await.is_err() {
                break;
            }
        }
    });

    let event_out = out_tx.clone();
    let forwarder = tauri::async_runtime::spawn(async move {
        loop {
            match event_rx.recv().await {
                Ok((name, payload)) => {
                    let msg = json!({ "type": "event", "event": name, "payload": payload });
                    if event_out.send(msg.to_string()).is_err() {
                        break;
                    }
                }
                Err(broadcast::error::RecvError::Lagged(skipped)) => {
                    tracing::warn!("WS 事件转发滞后，丢弃 {skipped} 条");
                }
                Err(broadcast::error::RecvError::Closed) => break,
            }
        }
    });

    while let Some(message) = source.next().await {
        let message = match message {
            Ok(m) => m,
            Err(_) => break,
        };
        match message {
            Message::Text(text) => {
                // 每条 invoke 独立任务执行，慢命令不阻塞连接读循环（与 Tauri IPC 并发语义一致）
                let app = app.clone();
                let out = out_tx.clone();
                tauri::async_runtime::spawn(async move {
                    if let Some(response) = handle_invoke(&app, &text).await {
                        let _ = out.send(response);
                    }
                });
            }
            Message::Close(_) => break,
            _ => {}
        }
    }

    writer.abort();
    forwarder.abort();
}

/// 解析一条入站消息并执行命令，返回应回写的 response JSON（非 invoke 消息返回 None）。
/// pub 仅供 tests/ 集成测试做协议级验证。
pub async fn handle_invoke<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    raw: &str,
) -> Option<String> {
    let message: InboundMessage = match serde_json::from_str(raw) {
        Ok(m) => m,
        Err(e) => {
            tracing::debug!("WS 消息解析失败: {e}");
            return None;
        }
    };
    if message.kind != "invoke" {
        return None;
    }
    let id = message.id?;
    let cmd = message.cmd.unwrap_or_default();
    let args = message.args.unwrap_or_else(|| json!({}));
    let response = match router::dispatch(app, &cmd, &args).await {
        Ok(data) => json!({ "type": "response", "id": id, "data": data }),
        Err(error) => json!({ "type": "response", "id": id, "error": error }),
    };
    Some(response.to_string())
}
