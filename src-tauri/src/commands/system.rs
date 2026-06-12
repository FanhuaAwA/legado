use crate::state::AppState;
use reader_core::CommandError;
use serde::Serialize;
use tauri::{Emitter, State};

#[tauri::command]
pub fn frontend_log(level: String, message: String) {
    match level.as_str() {
        "error" => tracing::error!(target: "frontend", "{message}"),
        "warning" => tracing::warn!(target: "frontend", "{message}"),
        "success" | "info" => tracing::info!(target: "frontend", "{message}"),
        _ => tracing::debug!(target: "frontend", level = %level, "{message}"),
    }
}

#[tauri::command]
pub fn get_platform() -> &'static str {
    #[cfg(target_os = "windows")]
    {
        "windows"
    }
    #[cfg(target_os = "macos")]
    {
        "macos"
    }
    #[cfg(target_os = "linux")]
    {
        "linux"
    }
    #[cfg(target_os = "android")]
    {
        "android"
    }
    #[cfg(target_os = "ios")]
    {
        "ios"
    }
    #[cfg(not(any(
        target_os = "windows",
        target_os = "macos",
        target_os = "linux",
        target_os = "android",
        target_os = "ios"
    )))]
    {
        "unknown"
    }
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct FeatureCapability {
    supported: bool,
    reason: &'static str,
    commands: Vec<&'static str>,
}

/// 单一能力域声明。新增功能模块时只需在 CAPABILITY_SPECS 追加一条记录，
/// 前后端契约（capabilities_get 返回的 key -> FeatureCapability 映射）自动扩展。
pub struct CapabilitySpec {
    pub key: &'static str,
    pub supported: bool,
    pub reason: &'static str,
    pub commands: &'static [&'static str],
}

pub const CAPABILITY_SPECS: &[CapabilitySpec] = &[
    CapabilitySpec {
        key: "syncWebdav",
        supported: true,
        reason: "WebDAV sync is implemented for credentials, connection test, status and manual sync.",
        commands: &[
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
        ],
    },
    CapabilitySpec {
        key: "sync",
        supported: false,
        reason: "Baidu Netdisk and FTP sync providers are not implemented in this build.",
        commands: &[
            "sync_baidu_start_auth",
            "sync_baidu_poll_token",
            "sync_baidu_token_status",
            "sync_baidu_revoke_auth",
        ],
    },
    CapabilitySpec {
        key: "tts",
        supported: false,
        reason: "Native TTS backend is not implemented in this build; browser speech remains available.",
        commands: &[
            "tts_get_voices",
            "tts_is_initialized",
            "tts_is_speaking",
            "tts_speak",
            "tts_stop",
            "tts_preview_voice",
        ],
    },
    CapabilitySpec {
        key: "videoProxy",
        supported: false,
        reason: "Local video proxy is not implemented in this build.",
        commands: &["start_video_proxy", "stop_video_proxy"],
    },
    CapabilitySpec {
        key: "browserProbe",
        supported: false,
        reason: "Headless browser probe is not implemented in this build; sources requiring WebView verification cannot run it.",
        commands: &[
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
        ],
    },
    CapabilitySpec {
        key: "comicCache",
        supported: false,
        reason: "Comic page cache is not implemented in this build; pages load directly from the network.",
        commands: &[
            "comic_cache_clear",
            "comic_cache_clear_chapter",
            "comic_cache_size",
            "comic_download_images",
            "comic_get_cached_page",
            "comic_get_page_sizes",
        ],
    },
    CapabilitySpec {
        key: "coverCache",
        supported: false,
        reason: "Cover disk cache is not implemented in this build; covers load directly from the network.",
        commands: &["cover_cache_clear", "cover_cache_size", "cover_resolve_cache"],
    },
    CapabilitySpec {
        key: "repository",
        supported: true,
        reason: "Source repository browsing and JS-source auto-update via @updateUrl are supported.",
        commands: &[
            "repository_fetch",
            "repository_install",
            "repository_preview_source",
            "repository_check_source_sync",
            "booksource_check_update",
            "booksource_apply_update",
        ],
    },
    CapabilitySpec {
        key: "unlock",
        supported: false,
        reason: "Secure-mode unlock challenges are not implemented in this build.",
        commands: &[
            "issue_full_mode_challenge",
            "verify_full_mode_challenge",
            "issue_scoped_unlock_challenge",
            "verify_scoped_unlock_challenge",
        ],
    },
    CapabilitySpec {
        key: "aiProxy",
        supported: true,
        reason: "AI HTTP proxy is available for whitelisted OpenAI-compatible model endpoints.",
        commands: &["ai_http_proxy_request"],
    },
    CapabilitySpec {
        key: "pluginHttp",
        supported: false,
        reason: "Frontend plugin HTTP bridge is not implemented in this build.",
        commands: &["frontend_plugin_http_request"],
    },
    CapabilitySpec {
        key: "exploreCache",
        supported: false,
        reason: "Explore result cache is not implemented in this build; nothing to clear.",
        commands: &["explore_clear_cache"],
    },
];

#[tauri::command]
pub fn capabilities_get() -> std::collections::BTreeMap<&'static str, FeatureCapability> {
    CAPABILITY_SPECS
        .iter()
        .map(|spec| {
            (
                spec.key,
                FeatureCapability {
                    supported: spec.supported,
                    reason: spec.reason,
                    commands: spec.commands.to_vec(),
                },
            )
        })
        .collect()
}

#[tauri::command]
pub async fn open_dir_in_explorer(path: String) -> Result<(), CommandError> {
    tauri_plugin_opener::open_path(path, None::<&str>).map_err(|err| CommandError {
        code: "IO_ERROR".to_string(),
        message: err.to_string(),
        detail: Some(format!("{err:?}")),
        retryable: false,
    })
}

#[tauri::command]
pub async fn script_dialog_result(
    app: tauri::AppHandle,
    id: String,
    value: serde_json::Value,
) -> Result<(), CommandError> {
    let _ = app.emit(
        "script:dialog:result",
        serde_json::json!({
            "id": id,
            "value": value
        }),
    );
    Ok(())
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioResolveRequest {
    pub url: String,
    pub referer: String,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioResolveRequestWrapper {
    pub request: AudioResolveRequest,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioResolveResponse {
    local_path: String,
}

#[tauri::command]
pub async fn audio_resolve_cache(
    state: State<'_, AppState>,
    request: AudioResolveRequestWrapper,
) -> Result<AudioResolveResponse, CommandError> {
    let local_path = state
        .core
        .resolve_audio_cache(&request.request.url, &request.request.referer)
        .await
        .map_err(|err| err.into_command_error())?;
    Ok(AudioResolveResponse { local_path })
}

#[tauri::command]
pub async fn script_repl_eval(
    state: State<'_, AppState>,
    code: String,
    context_file: Option<String>,
) -> Result<String, CommandError> {
    state
        .core
        .eval_repl(&code, context_file.as_deref())
        .map_err(|err| err.into_command_error())
}
