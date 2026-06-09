use reader_core::CommandError;

type CommandResult<T> = Result<T, CommandError>;
fn u(f: &str) -> CommandError { CommandError { code: "UNSUPPORTED".into(), message: format!("{f} 功能尚未实现"), detail: None, retryable: false } }

// ── 同步（百度网盘 / 通用云同步）───────────────────────────
#[tauri::command] pub async fn sync_baidu_start_auth() -> CommandResult<()> { Err(u("百度网盘授权")) }
#[tauri::command] pub async fn sync_baidu_poll_token() -> CommandResult<()> { Err(u("百度网盘授权")) }
#[tauri::command] pub async fn sync_baidu_token_status() -> CommandResult<()> { Err(u("百度网盘授权")) }
#[tauri::command] pub async fn sync_baidu_revoke_auth() -> CommandResult<()> { Err(u("百度网盘授权")) }
#[tauri::command] pub async fn sync_set_credentials() -> CommandResult<()> { Err(u("云同步凭据")) }
#[tauri::command] pub async fn sync_get_credentials() -> CommandResult<()> { Err(u("云同步凭据")) }
#[tauri::command] pub async fn sync_clear_credentials() -> CommandResult<()> { Err(u("云同步凭据")) }
#[tauri::command] pub async fn sync_get_status() -> CommandResult<()> { Err(u("云同步状态")) }
#[tauri::command] pub async fn sync_now() -> CommandResult<()> { Err(u("云同步")) }
#[tauri::command] pub async fn sync_test_connection() -> CommandResult<()> { Err(u("云同步")) }
#[tauri::command] pub async fn sync_list_conflicts() -> CommandResult<()> { Err(u("云同步")) }
#[tauri::command] pub async fn sync_resolve_conflict() -> CommandResult<()> { Err(u("云同步")) }
#[tauri::command] pub async fn sync_notify_lifecycle() -> CommandResult<()> { Err(u("云同步")) }
#[tauri::command] pub async fn sync_client_state_set() -> CommandResult<()> { Err(u("云同步")) }
#[tauri::command] pub async fn sync_report_reader_session() -> CommandResult<()> { Err(u("云同步")) }
#[tauri::command] pub async fn sync_v2_sync_reading_progress() -> CommandResult<()> { Err(u("云同步")) }

// ── TTS ───────────────────────────────────────────────────
#[tauri::command] pub async fn tts_get_voices() -> CommandResult<()> { Err(u("TTS 语音合成")) }
#[tauri::command] pub async fn tts_is_initialized() -> CommandResult<()> { Err(u("TTS 语音合成")) }
#[tauri::command] pub async fn tts_is_speaking() -> CommandResult<()> { Err(u("TTS 语音合成")) }
#[tauri::command] pub async fn tts_speak() -> CommandResult<()> { Err(u("TTS 语音合成")) }
#[tauri::command] pub async fn tts_stop() -> CommandResult<()> { Err(u("TTS 语音合成")) }
#[tauri::command] pub async fn tts_preview_voice() -> CommandResult<()> { Err(u("TTS 语音合成")) }

// ── 视频代理 ──────────────────────────────────────────────
#[tauri::command] pub async fn start_video_proxy() -> CommandResult<()> { Err(u("视频代理")) }
#[tauri::command] pub async fn stop_video_proxy() -> CommandResult<()> { Err(u("视频代理")) }

// ── Web 服务 ──────────────────────────────────────────────
#[tauri::command] pub async fn web_server_pick_dist_dir() -> CommandResult<()> { Err(u("Web 服务")) }
#[tauri::command] pub async fn web_server_start() -> CommandResult<()> { Err(u("Web 服务")) }
#[tauri::command] pub async fn web_server_stop() -> CommandResult<()> { Err(u("Web 服务")) }
#[tauri::command] pub async fn web_server_status() -> CommandResult<()> { Err(u("Web 服务")) }

// ── 解锁 ──────────────────────────────────────────────────
#[tauri::command] pub async fn issue_full_mode_challenge() -> CommandResult<()> { Err(u("解锁")) }
#[tauri::command] pub async fn issue_scoped_unlock_challenge() -> CommandResult<()> { Err(u("解锁")) }
#[tauri::command] pub async fn verify_full_mode_challenge() -> CommandResult<()> { Err(u("解锁")) }
#[tauri::command] pub async fn verify_scoped_unlock_challenge() -> CommandResult<()> { Err(u("解锁")) }

// ── 书源仓库 ──────────────────────────────────────────────
#[tauri::command] pub async fn repository_check_source_sync() -> CommandResult<()> { Err(u("书源仓库")) }
#[tauri::command] pub async fn repository_fetch() -> CommandResult<()> { Err(u("书源仓库")) }
#[tauri::command] pub async fn repository_install() -> CommandResult<()> { Err(u("书源仓库")) }
#[tauri::command] pub async fn repository_preview_source() -> CommandResult<()> { Err(u("书源仓库")) }

// ── 杂项 ──────────────────────────────────────────────────
#[tauri::command] pub async fn ai_http_proxy_url() -> CommandResult<()> { Err(u("AI HTTP 代理")) }
#[tauri::command] pub async fn app_update_download() -> CommandResult<()> { Err(u("应用更新")) }
#[tauri::command] pub async fn app_update_install_downloaded_file() -> CommandResult<()> { Err(u("应用更新")) }
#[tauri::command] pub async fn get_local_ips() -> CommandResult<()> { Err(u("局域网信息")) }
#[tauri::command] pub async fn frontend_plugin_http_request() -> CommandResult<()> { Err(u("前端插件 HTTP")) }
#[tauri::command] pub async fn explore_clear_cache() -> CommandResult<()> { Err(u("发现页缓存清理")) }
