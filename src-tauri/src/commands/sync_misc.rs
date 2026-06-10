use reader_core::CommandError;
use serde::Serialize;
use std::sync::Mutex;

type CommandResult<T> = Result<T, CommandError>;
fn u(f: &str) -> CommandError {
    CommandError {
        code: "UNSUPPORTED".into(),
        message: format!("{f} 功能尚未实现"),
        detail: None,
        retryable: false,
    }
}

// ── 同步 ──────────────────────────────────────────────────
#[tauri::command]
pub async fn sync_baidu_start_auth() -> CommandResult<()> {
    Err(u("百度网盘授权"))
}
#[tauri::command]
pub async fn sync_baidu_poll_token() -> CommandResult<()> {
    Err(u("百度网盘授权"))
}
#[tauri::command]
pub async fn sync_baidu_token_status() -> CommandResult<()> {
    Err(u("百度网盘授权"))
}
#[tauri::command]
pub async fn sync_baidu_revoke_auth() -> CommandResult<()> {
    Err(u("百度网盘授权"))
}
#[tauri::command]
pub async fn sync_set_credentials() -> CommandResult<()> {
    Err(u("云同步凭据"))
}
#[tauri::command]
pub async fn sync_get_credentials() -> CommandResult<()> {
    Err(u("云同步凭据"))
}
#[tauri::command]
pub async fn sync_clear_credentials() -> CommandResult<()> {
    Err(u("云同步凭据"))
}
#[tauri::command]
pub async fn sync_get_status() -> CommandResult<()> {
    Err(u("云同步状态"))
}
#[tauri::command]
pub async fn sync_now() -> CommandResult<()> {
    Err(u("云同步"))
}
#[tauri::command]
pub async fn sync_test_connection() -> CommandResult<()> {
    Err(u("云同步"))
}
#[tauri::command]
pub async fn sync_list_conflicts() -> CommandResult<()> {
    Err(u("云同步"))
}
#[tauri::command]
pub async fn sync_resolve_conflict() -> CommandResult<()> {
    Err(u("云同步"))
}
#[tauri::command]
pub async fn sync_notify_lifecycle() -> CommandResult<()> {
    Err(u("云同步"))
}
#[tauri::command]
pub async fn sync_client_state_set() -> CommandResult<()> {
    Err(u("云同步"))
}
#[tauri::command]
pub async fn sync_report_reader_session() -> CommandResult<()> {
    Err(u("云同步"))
}
#[tauri::command]
pub async fn sync_v2_sync_reading_progress() -> CommandResult<()> {
    Err(u("云同步"))
}

// ── TTS ───────────────────────────────────────────────────
#[tauri::command]
pub async fn tts_get_voices() -> CommandResult<()> {
    Err(u("TTS 语音合成"))
}
#[tauri::command]
pub async fn tts_is_initialized() -> CommandResult<()> {
    Err(u("TTS 语音合成"))
}
#[tauri::command]
pub async fn tts_is_speaking() -> CommandResult<()> {
    Err(u("TTS 语音合成"))
}
#[tauri::command]
pub async fn tts_speak() -> CommandResult<()> {
    Err(u("TTS 语音合成"))
}
#[tauri::command]
pub async fn tts_stop() -> CommandResult<()> {
    Err(u("TTS 语音合成"))
}
#[tauri::command]
pub async fn tts_preview_voice() -> CommandResult<()> {
    Err(u("TTS 语音合成"))
}

// ── 视频代理 ──────────────────────────────────────────────
#[tauri::command]
pub async fn start_video_proxy() -> CommandResult<()> {
    Err(u("视频代理"))
}
#[tauri::command]
pub async fn stop_video_proxy() -> CommandResult<()> {
    Err(u("视频代理"))
}

// ── Web 服务 ──────────────────────────────────────────────
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// 运行中的 Web 服务句柄。
///
/// 关键：监听套接字由服务线程独占持有，静态区只保留端口和关闭信号，不再克隆
/// 套接字。旧实现 `try_clone()` 监听器后让 stop 仅 drop 原始句柄，克隆 fd 仍存活，
/// 端口永不释放、线程永不退出。现改为非阻塞 accept 轮询 + 关闭标志：stop 置位后
/// 线程在一个轮询周期内退出并 drop 监听器，端口真实释放。
struct WebServerHandle {
    port: u16,
    shutdown: Arc<AtomicBool>,
}

static WEB_SERVER: Mutex<Option<WebServerHandle>> = Mutex::new(None);

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WebServerStatus {
    pub running: bool,
    pub port: u16,
    pub dist_dir: Option<String>,
}

static DIST_DIR: Mutex<Option<String>> = Mutex::new(None);

#[tauri::command]
pub fn web_server_pick_dist_dir(app: tauri::AppHandle) -> CommandResult<Option<String>> {
    #[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
    {
        use tauri_plugin_dialog::DialogExt;
        let result = app.dialog().file().blocking_pick_folder();
        if let Some(path) = result {
            let p = path.to_string();
            if let Ok(mut d) = DIST_DIR.lock() {
                *d = Some(p.clone());
            }
            return Ok(Some(p));
        }
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    let _ = app;
    Ok(None)
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebServerStartRequest {
    pub port: Option<u16>,
}

/// 启动 Web 服务（内部实现，与 Tauri command 分离以便单元测试）。
fn start_web_server(req_port: u16) -> CommandResult<WebServerStatus> {
    let mut guard = WEB_SERVER.lock().map_err(|e| CommandError {
        code: "IO_ERROR".into(),
        message: e.to_string(),
        detail: None,
        retryable: false,
    })?;
    if let Some(handle) = guard.as_ref() {
        let dir = DIST_DIR.lock().ok().and_then(|d| d.clone());
        return Ok(WebServerStatus {
            running: true,
            port: handle.port,
            dist_dir: dir,
        });
    }
    let dist = DIST_DIR
        .lock()
        .ok()
        .and_then(|d| d.clone())
        .unwrap_or_else(|| ".".to_string());
    let bind_addr = if req_port > 0 {
        format!("0.0.0.0:{req_port}")
    } else {
        "127.0.0.1:0".into()
    };
    let listener = std::net::TcpListener::bind(&bind_addr).map_err(|e| CommandError {
        code: "IO_ERROR".into(),
        message: format!("无法启动 Web 服务: {e}"),
        detail: None,
        retryable: false,
    })?;
    let port = listener.local_addr().map(|a| a.port()).unwrap_or(0);
    listener.set_nonblocking(true).map_err(|e| CommandError {
        code: "IO_ERROR".into(),
        message: format!("无法设置非阻塞监听: {e}"),
        detail: None,
        retryable: false,
    })?;

    let shutdown = Arc::new(AtomicBool::new(false));
    let shutdown_thread = shutdown.clone();
    let serve_dir = dist.clone();
    std::thread::spawn(move || {
        // listener 由本线程独占；循环退出后随作用域 drop，套接字关闭、端口释放。
        loop {
            if shutdown_thread.load(Ordering::Relaxed) {
                break;
            }
            match listener.accept() {
                Ok((stream, _)) => {
                    let _ = tiny_http(stream, &serve_dir);
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(std::time::Duration::from_millis(50));
                }
                Err(_) => {
                    std::thread::sleep(std::time::Duration::from_millis(50));
                }
            }
        }
    });

    *guard = Some(WebServerHandle { port, shutdown });
    Ok(WebServerStatus {
        running: true,
        port,
        dist_dir: Some(dist),
    })
}

/// 停止 Web 服务（内部实现）。置位关闭标志并取出句柄；服务线程在下一个轮询周期
/// （≤50ms）退出并释放端口。
fn stop_web_server() -> CommandResult<WebServerStatus> {
    let mut guard = WEB_SERVER.lock().map_err(|e| CommandError {
        code: "IO_ERROR".into(),
        message: e.to_string(),
        detail: None,
        retryable: false,
    })?;
    if let Some(handle) = guard.take() {
        handle.shutdown.store(true, Ordering::Relaxed);
    }
    let dir = DIST_DIR.lock().ok().and_then(|d| d.clone());
    Ok(WebServerStatus {
        running: false,
        port: 0,
        dist_dir: dir,
    })
}

#[tauri::command]
pub fn web_server_start(request: Option<WebServerStartRequest>) -> CommandResult<WebServerStatus> {
    start_web_server(request.and_then(|r| r.port).unwrap_or(0))
}

#[tauri::command]
pub fn web_server_stop() -> CommandResult<WebServerStatus> {
    stop_web_server()
}

#[tauri::command]
pub fn web_server_status() -> CommandResult<WebServerStatus> {
    let guard = WEB_SERVER.lock().map_err(|e| CommandError {
        code: "IO_ERROR".into(),
        message: e.to_string(),
        detail: None,
        retryable: false,
    })?;
    if let Some(handle) = guard.as_ref() {
        let dir = DIST_DIR.lock().ok().and_then(|d| d.clone());
        Ok(WebServerStatus {
            running: true,
            port: handle.port,
            dist_dir: dir,
        })
    } else {
        Ok(WebServerStatus {
            running: false,
            port: 0,
            dist_dir: None,
        })
    }
}

fn tiny_http(mut stream: std::net::TcpStream, serve_dir: &str) -> std::io::Result<()> {
    use std::io::{BufRead, BufReader, Write};
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut request_line = String::new();
    reader.read_line(&mut request_line)?;
    let parts: Vec<&str> = request_line.split_whitespace().collect();
    let path = if parts.len() >= 2 { parts[1] } else { "/" };
    let path = if path == "/" { "/index.html" } else { path };
    let file_path = std::path::PathBuf::from(serve_dir).join(path.trim_start_matches('/'));
    let mime = match file_path.extension().and_then(|e| e.to_str()) {
        Some("html") => "text/html",
        Some("css") => "text/css",
        Some("js") => "application/javascript",
        Some("json") => "application/json",
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("svg") => "image/svg+xml",
        Some("woff2") => "font/woff2",
        _ => "application/octet-stream",
    };
    let (status, body) = if file_path.exists() && file_path.is_file() {
        match std::fs::read(&file_path) {
            Ok(data) => (200, data),
            Err(_) => (404, b"Not Found".to_vec()),
        }
    } else {
        (404, b"Not Found".to_vec())
    };
    let content_type = if status == 200 { mime } else { "text/plain" };
    let response = format!(
        "HTTP/1.1 {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\nConnection: close\r\n\r\n",
        status, content_type, body.len()
    );
    stream.write_all(response.as_bytes())?;
    stream.write_all(&body)?;
    Ok(())
}

// ── 局域网信息 ────────────────────────────────────────────

#[tauri::command]
pub fn get_local_ips() -> CommandResult<Vec<String>> {
    let mut ips = Vec::new();
    // Try PowerShell on Windows
    #[cfg(target_os = "windows")]
    {
        if let Ok(output) = std::process::Command::new("powershell")
            .args(["-NoProfile", "-Command",
                "Get-NetIPAddress -AddressFamily IPv4 | Where-Object { $_.InterfaceAlias -notmatch 'Loopback' -and $_.IPAddress -ne '127.0.0.1' } | ForEach-Object { '{0}|{1}' -f $_.InterfaceAlias, $_.IPAddress }"])
            .output()
        {
            if output.status.success() {
                for line in String::from_utf8_lossy(&output.stdout).lines() {
                    let line = line.trim();
                    if let Some((_name, addr)) = line.split_once('|') {
                        ips.push(addr.to_string());
                    }
                }
            }
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        // Fallback: use hostname
        if let Ok(host) = std::process::Command::new("hostname").arg("-I").output() {
            for addr in String::from_utf8_lossy(&host.stdout).split_whitespace() {
                ips.push(addr.to_string());
            }
        }
    }
    if ips.is_empty() {
        ips.push("127.0.0.1".to_string());
    }
    Ok(ips)
}

// ── 解锁 ──────────────────────────────────────────────────
#[tauri::command]
pub async fn issue_full_mode_challenge() -> CommandResult<()> {
    Err(u("解锁"))
}
#[tauri::command]
pub async fn issue_scoped_unlock_challenge() -> CommandResult<()> {
    Err(u("解锁"))
}
#[tauri::command]
pub async fn verify_full_mode_challenge() -> CommandResult<()> {
    Err(u("解锁"))
}
#[tauri::command]
pub async fn verify_scoped_unlock_challenge() -> CommandResult<()> {
    Err(u("解锁"))
}

// ── 书源仓库 ──────────────────────────────────────────────
#[tauri::command]
pub async fn repository_check_source_sync() -> CommandResult<()> {
    Err(u("书源仓库"))
}
#[tauri::command]
pub async fn repository_fetch() -> CommandResult<()> {
    Err(u("书源仓库"))
}
#[tauri::command]
pub async fn repository_install() -> CommandResult<()> {
    Err(u("书源仓库"))
}
#[tauri::command]
pub async fn repository_preview_source() -> CommandResult<()> {
    Err(u("书源仓库"))
}

// ── 杂项 ──────────────────────────────────────────────────
#[tauri::command]
pub async fn ai_http_proxy_url() -> CommandResult<()> {
    Err(u("AI HTTP 代理"))
}
#[tauri::command]
pub async fn app_update_download() -> CommandResult<()> {
    Err(u("应用更新"))
}
#[tauri::command]
pub async fn app_update_install_downloaded_file() -> CommandResult<()> {
    Err(u("应用更新"))
}
#[tauri::command]
pub async fn frontend_plugin_http_request() -> CommandResult<()> {
    Err(u("前端插件 HTTP"))
}
#[tauri::command]
pub async fn explore_clear_cache() -> CommandResult<()> {
    Err(u("发现页缓存清理"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 回归（R-P1-002）：start → stop → 同端口再 start 必须成功。
    /// 旧实现 stop 只 drop 原始监听器、克隆 fd 仍存活，端口不释放，
    /// 第二次同端口 start 会 AddrInUse 失败。
    #[test]
    fn web_server_stop_releases_port_for_restart() {
        // 选一个大概率空闲的固定端口；先确保干净状态。
        let _ = stop_web_server();
        let port: u16 = 39517;

        let s1 = start_web_server(port).expect("首次启动应成功");
        assert!(s1.running);
        assert_eq!(s1.port, port, "应使用请求端口");

        let stopped = stop_web_server().expect("停止应成功");
        assert!(!stopped.running);

        // 给服务线程一个轮询周期退出并释放端口。
        std::thread::sleep(std::time::Duration::from_millis(200));

        let s2 = start_web_server(port).expect("停止后同端口应能再次绑定（端口已真实释放）");
        assert!(s2.running);
        assert_eq!(s2.port, port);

        let _ = stop_web_server();
    }
}
