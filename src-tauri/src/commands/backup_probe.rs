use crate::state::AppState;
use reader_core::CommandError;
use serde::{Deserialize, Serialize};
use tauri::State;

type CommandResult<T> = Result<T, CommandError>;

fn unsupported(f: &str) -> CommandError {
    CommandError { code: "UNSUPPORTED".into(), message: format!("{f} 功能尚未实现"), detail: None, retryable: false }
}

// ── DTOs (must be pub for tauri::command) ─────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupCategoryStat {
    pub id: String,
    pub label: String,
    pub description: String,
    pub item_count: i64,
    pub byte_size: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupInspectReport {
    pub categories: Vec<BackupCategoryStat>,
    pub total_bytes: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupManifest {
    pub format: String,
    pub version: i32,
    pub created_at: i64,
    pub app_version: String,
    pub categories: Vec<BackupCategoryStat>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupCreateResult {
    pub output_path: String,
    pub byte_size: i64,
    pub categories: Vec<BackupCategoryStat>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupCreateDataResult {
    pub file_name: String,
    pub mime: String,
    pub base64: String,
    pub byte_size: i64,
    pub categories: Vec<BackupCategoryStat>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupPeekReport {
    pub manifest: BackupManifest,
    pub unknown_categories: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupRestoreResult {
    pub restored: Vec<BackupCategoryStat>,
    pub skipped: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupCreateRequest {
    pub output_path: String,
    pub categories: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupCreateDataRequest {
    pub default_name: String,
    pub categories: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupPeekRequest {
    pub json_path: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupPeekDataRequest {
    pub base64: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupRestoreRequest {
    pub json_path: String,
    pub categories: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupRestoreDataRequest {
    pub base64: String,
    pub categories: Vec<String>,
}

// ── Helpers ─────────────────────────────────────────────

const LABELS: &[(&str, &str, &str)] = &[
    ("app_settings", "应用设置", "前端配置和应用偏好"),
    ("reader_settings", "阅读设置", "字体、主题、翻页等偏好"),
    ("bookshelf", "书架", "书架书籍元数据和阅读进度"),
    ("bookshelf_cache", "章节缓存", "已下载的章节正文缓存"),
    ("booksources", "书源", "已安装的书源文件和配置"),
    ("extensions", "扩展", "已安装的扩展"),
    ("script_config", "脚本配置", "脚本和前端存储配置"),
    ("sync_state", "同步状态", "云同步状态数据"),
    ("user_fonts", "用户字体", "用户上传的自定义字体"),
    ("other_frontend", "其他前端数据", "其他前端存储数据"),
];

fn build_stats(state: &AppState) -> std::io::Result<Vec<BackupCategoryStat>> {
    let reader_dir = state.core.reader_dir();
    let mut stats = Vec::new();
    for (id, label, desc) in LABELS {
        let (count, bytes) = match *id {
            "app_settings" | "script_config" => {
                let c = count_files(&reader_dir.join("config")).unwrap_or(0) as i64;
                let s = count_dir_size(&reader_dir.join("config")).unwrap_or(0) as i64;
                (c, s)
            }
            "bookshelf" => {
                let s = reader_dir.join("data").join("local").join("shelf.json");
                let sz = std::fs::metadata(&s).map(|m| m.len() as i64).unwrap_or(0);
                (1, sz)
            }
            "booksources" => {
                let l = reader_dir.join("sources").join("legado-json");
                let j = reader_dir.join("sources").join("script-js");
                let c = count_files(&l).unwrap_or(0) + count_files(&j).unwrap_or(0);
                let s = count_dir_size(&l).unwrap_or(0) + count_dir_size(&j).unwrap_or(0);
                (c as i64, s as i64)
            }
            _ => (0, 0),
        };
        stats.push(BackupCategoryStat {
            id: id.to_string(), label: label.to_string(), description: desc.to_string(),
            item_count: count, byte_size: bytes,
        });
    }
    Ok(stats)
}

fn count_dir_size(path: &std::path::Path) -> std::io::Result<u64> {
    if !path.exists() { return Ok(0); }
    let mut total = 0u64;
    for entry in std::fs::read_dir(path)? {
        let e = entry?;
        let p = e.path();
        if p.is_file() { total += e.metadata()?.len(); }
        else if p.is_dir() { total += count_dir_size(&p)?; }
    }
    Ok(total)
}

fn count_files(path: &std::path::Path) -> std::io::Result<usize> {
    if !path.exists() { return Ok(0); }
    let mut c = 0usize;
    for entry in std::fs::read_dir(path)? {
        let e = entry?;
        let p = e.path();
        if p.is_file() { c += 1; }
        else if p.is_dir() { c += count_files(&p)?; }
    }
    Ok(c)
}

fn filter_stats(stats: &[BackupCategoryStat], categories: &[String]) -> Vec<BackupCategoryStat> {
    if categories.is_empty() || categories.iter().any(|c| c == "all") {
        return stats.to_vec();
    }
    stats.iter().filter(|s| categories.contains(&s.id)).cloned().collect()
}

fn build_manifest(stats: &[BackupCategoryStat]) -> BackupManifest {
    BackupManifest {
        format: "legado-backup-v1".into(), version: 1,
        created_at: chrono::Utc::now().timestamp_millis(),
        app_version: env!("CARGO_PKG_VERSION").into(),
        categories: stats.to_vec(),
    }
}

fn pack_categories(state: &AppState, selected: &[BackupCategoryStat]) -> Result<serde_json::Value, CommandError> {
    let reader_dir = state.core.reader_dir();
    let mut payload = serde_json::Map::new();
    let manifest = build_manifest(selected);
    payload.insert("manifest".into(), serde_json::to_value(&manifest).map_err(|e| CommandError {
        code: "IO_ERROR".into(), message: e.to_string(), detail: None, retryable: false,
    })?);
    let mut data = serde_json::Map::new();
    for s in selected {
        match s.id.as_str() {
            "app_settings" | "script_config" => {
                let d = reader_dir.join("config");
                if d.exists() {
                    let mut cfg = serde_json::Map::new();
                    if let Ok(entries) = std::fs::read_dir(&d) {
                        for entry in entries.flatten() {
                            let p = entry.path();
                            if p.extension().and_then(|e| e.to_str()) == Some("json") {
                                if let Ok(raw) = std::fs::read_to_string(&p) {
                                    let key = p.file_stem().unwrap_or_default().to_string_lossy().to_string();
                                    if let Ok(v) = serde_json::from_str::<serde_json::Value>(&raw) {
                                        cfg.insert(key, v);
                                    }
                                }
                            }
                        }
                    }
                    data.insert(s.id.clone(), serde_json::Value::Object(cfg));
                }
            }
            "bookshelf" => {
                let sf = reader_dir.join("data").join("local").join("shelf.json");
                if let Ok(raw) = std::fs::read_to_string(&sf) {
                    if let Ok(v) = serde_json::from_str::<serde_json::Value>(&raw) {
                        data.insert("bookshelf".into(), v);
                    }
                }
            }
            "booksources" => {
                let l = reader_dir.join("sources").join("legado-json");
                let mut srcs = Vec::new();
                if let Ok(entries) = std::fs::read_dir(&l) {
                    for entry in entries.flatten() {
                        if let Ok(raw) = std::fs::read_to_string(&entry.path()) {
                            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&raw) {
                                srcs.push(v);
                            }
                        }
                    }
                }
                data.insert("booksources".into(), serde_json::Value::Array(srcs));
            }
            _ => {}
        }
    }
    payload.insert("data".into(), serde_json::Value::Object(data));
    Ok(serde_json::Value::Object(payload))
}

// ── Commands ────────────────────────────────────────────

#[tauri::command]
pub async fn backup_inspect(state: State<'_, AppState>) -> CommandResult<BackupInspectReport> {
    let stats = build_stats(&state).map_err(|e| CommandError { code: "IO_ERROR".into(), message: e.to_string(), detail: None, retryable: false })?;
    let total = stats.iter().map(|s| s.byte_size).sum();
    Ok(BackupInspectReport { categories: stats, total_bytes: total })
}

#[tauri::command]
pub async fn backup_create(state: State<'_, AppState>, request: BackupCreateRequest) -> CommandResult<BackupCreateResult> {
    let all = build_stats(&state).map_err(|e| CommandError { code: "IO_ERROR".into(), message: e.to_string(), detail: None, retryable: false })?;
    let selected = filter_stats(&all, &request.categories);
    let payload = pack_categories(&state, &selected)?;
    let json = serde_json::to_string_pretty(&payload).map_err(|e| CommandError { code: "IO_ERROR".into(), message: e.to_string(), detail: None, retryable: false })?;
    std::fs::write(&request.output_path, &json).map_err(|e| CommandError { code: "IO_ERROR".into(), message: e.to_string(), detail: None, retryable: false })?;
    let size = json.len() as i64;
    Ok(BackupCreateResult { output_path: request.output_path, byte_size: size, categories: selected })
}

#[tauri::command]
pub async fn backup_create_data(state: State<'_, AppState>, request: BackupCreateDataRequest) -> CommandResult<BackupCreateDataResult> {
    let all = build_stats(&state).map_err(|e| CommandError { code: "IO_ERROR".into(), message: e.to_string(), detail: None, retryable: false })?;
    let selected = filter_stats(&all, &request.categories);
    let payload = pack_categories(&state, &selected)?;
    let json = serde_json::to_string_pretty(&payload).map_err(|e| CommandError { code: "IO_ERROR".into(), message: e.to_string(), detail: None, retryable: false })?;
    use base64::Engine as _;
    let b64 = base64::engine::general_purpose::STANDARD.encode(&json);
    let size = json.len() as i64;
    Ok(BackupCreateDataResult { file_name: request.default_name, mime: "application/json".into(), base64: b64, byte_size: size, categories: selected })
}

#[tauri::command]
pub async fn backup_peek(request: BackupPeekRequest) -> CommandResult<BackupPeekReport> {
    let raw = std::fs::read_to_string(&request.json_path).map_err(|e| CommandError { code: "IO_ERROR".into(), message: e.to_string(), detail: None, retryable: false })?;
    parse_peek(&raw)
}

#[tauri::command]
pub async fn backup_peek_data(request: BackupPeekDataRequest) -> CommandResult<BackupPeekReport> {
    use base64::Engine as _;
    let bytes = base64::engine::general_purpose::STANDARD.decode(&request.base64).map_err(|e| CommandError { code: "IO_ERROR".into(), message: e.to_string(), detail: None, retryable: false })?;
    let raw = String::from_utf8(bytes).map_err(|e| CommandError { code: "IO_ERROR".into(), message: e.to_string(), detail: None, retryable: false })?;
    parse_peek(&raw)
}

fn parse_peek(raw: &str) -> CommandResult<BackupPeekReport> {
    let payload: serde_json::Value = serde_json::from_str(raw).map_err(|e| CommandError { code: "IO_ERROR".into(), message: format!("备份文件格式无效: {e}"), detail: None, retryable: false })?;
    let manifest: BackupManifest = serde_json::from_value(payload.get("manifest").cloned().unwrap_or_default()).map_err(|e| CommandError { code: "IO_ERROR".into(), message: e.to_string(), detail: None, retryable: false })?;
    let known: Vec<String> = LABELS.iter().map(|(id, _, _)| id.to_string()).collect();
    let unknown: Vec<String> = manifest.categories.iter().map(|c| c.id.clone()).filter(|id| !known.contains(id)).collect();
    Ok(BackupPeekReport { manifest, unknown_categories: unknown })
}

#[tauri::command]
pub async fn backup_restore(state: State<'_, AppState>, request: BackupRestoreRequest) -> CommandResult<BackupRestoreResult> {
    let raw = std::fs::read_to_string(&request.json_path).map_err(|e| CommandError { code: "IO_ERROR".into(), message: e.to_string(), detail: None, retryable: false })?;
    restore_from_payload(&state, &raw, &request.categories)
}

#[tauri::command]
pub async fn backup_restore_data(state: State<'_, AppState>, request: BackupRestoreDataRequest) -> CommandResult<BackupRestoreResult> {
    use base64::Engine as _;
    let bytes = base64::engine::general_purpose::STANDARD.decode(&request.base64).map_err(|e| CommandError { code: "IO_ERROR".into(), message: e.to_string(), detail: None, retryable: false })?;
    let raw = String::from_utf8(bytes).map_err(|e| CommandError { code: "IO_ERROR".into(), message: e.to_string(), detail: None, retryable: false })?;
    restore_from_payload(&state, &raw, &request.categories)
}

fn restore_from_payload(state: &AppState, raw: &str, categories: &[String]) -> CommandResult<BackupRestoreResult> {
    let payload: serde_json::Value = serde_json::from_str(raw).map_err(|e| CommandError { code: "IO_ERROR".into(), message: format!("备份文件格式无效: {e}"), detail: None, retryable: false })?;
    let data = payload.get("data").ok_or_else(|| CommandError { code: "IO_ERROR".into(), message: "备份文件缺少 data 字段".into(), detail: None, retryable: false })?;
    let reader_dir = state.core.reader_dir();
    let mut restored = Vec::new();
    let mut skipped = Vec::new();
    let filter: Vec<String> = if categories.is_empty() || categories.iter().any(|c| c == "all") { LABELS.iter().map(|(id, _, _)| id.to_string()).collect() } else { categories.to_vec() };

    for cat in &filter {
        match cat.as_str() {
            "app_settings" | "script_config" => {
                if let Some(cfg) = data.get(cat) {
                    let d = reader_dir.join("config");
                    let _ = std::fs::create_dir_all(&d);
                    if let serde_json::Value::Object(map) = cfg {
                        for (k, v) in map {
                            let p = d.join(format!("{}.json", k));
                            if let Ok(json) = serde_json::to_string_pretty(v) {
                                if std::fs::write(&p, &json).is_ok() {
                                    restored.push(BackupCategoryStat { id: cat.clone(), label: cat.clone(), description: String::new(), item_count: 1, byte_size: json.len() as i64 });
                                }
                            }
                        }
                    }
                }
            }
            "bookshelf" => {
                if let Some(shelf) = data.get("bookshelf") {
                    let sp = reader_dir.join("data").join("local").join("shelf.json");
                    if let Some(parent) = sp.parent() { let _ = std::fs::create_dir_all(parent); }
                    if let Ok(json) = serde_json::to_string_pretty(shelf) {
                        if std::fs::write(&sp, &json).is_ok() {
                            restored.push(BackupCategoryStat { id: cat.clone(), label: cat.clone(), description: String::new(), item_count: 1, byte_size: json.len() as i64 });
                        }
                    }
                }
            }
            "booksources" => {
                if let Some(srcs) = data.get("booksources").and_then(|v| v.as_array()) {
                    let d = reader_dir.join("sources").join("legado-json");
                    let _ = std::fs::create_dir_all(&d);
                    let mut count = 0i64;
                    let mut bytes = 0i64;
                    for s in srcs {
                        let name = s.get("bookSourceName").and_then(|v| v.as_str()).unwrap_or("unknown");
                        let fnm = format!("{}.legado.json", name.replace(['/', '\\', ':', '?', '*', '"', '<', '>', '|'], "_"));
                        if let Ok(json) = serde_json::to_string_pretty(s) {
                            bytes += json.len() as i64;
                            if std::fs::write(d.join(&fnm), &json).is_ok() { count += 1; }
                        }
                    }
                    restored.push(BackupCategoryStat { id: cat.clone(), label: cat.clone(), description: String::new(), item_count: count, byte_size: bytes });
                }
            }
            _ => { skipped.push(cat.clone()); }
        }
    }
    Ok(BackupRestoreResult { restored, skipped })
}

// ── Browser Probe stubs ─────────────────────────────────
#[tauri::command] pub async fn browser_probe_create() -> CommandResult<()> { Err(unsupported("浏览器探测")) }
#[tauri::command] pub async fn browser_probe_close() -> CommandResult<()> { Err(unsupported("浏览器探测")) }
#[tauri::command] pub async fn browser_probe_close_all() -> CommandResult<()> { Err(unsupported("浏览器探测")) }
#[tauri::command] pub async fn browser_probe_hide() -> CommandResult<()> { Err(unsupported("浏览器探测")) }
#[tauri::command] pub async fn browser_probe_show() -> CommandResult<()> { Err(unsupported("浏览器探测")) }
#[tauri::command] pub async fn browser_probe_navigate() -> CommandResult<()> { Err(unsupported("浏览器探测")) }
#[tauri::command] pub async fn browser_probe_eval() -> CommandResult<()> { Err(unsupported("浏览器探测")) }
#[tauri::command] pub async fn browser_probe_run() -> CommandResult<()> { Err(unsupported("浏览器探测")) }
#[tauri::command] pub async fn browser_probe_get_cookies() -> CommandResult<()> { Err(unsupported("浏览器探测")) }
#[tauri::command] pub async fn browser_probe_set_cookie() -> CommandResult<()> { Err(unsupported("浏览器探测")) }
#[tauri::command] pub async fn browser_probe_clear_data() -> CommandResult<()> { Err(unsupported("浏览器探测")) }
#[tauri::command] pub async fn browser_probe_set_user_agent() -> CommandResult<()> { Err(unsupported("浏览器探测")) }
