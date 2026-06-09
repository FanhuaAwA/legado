use crate::state::AppState;
use reader_core::CommandError;
use serde::Serialize;
use std::time::UNIX_EPOCH;
use tauri::State;
use tokio::fs;

type CommandResult<T> = Result<T, CommandError>;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionMeta {
    pub file_name: String,
    pub name: String,
    pub namespace: String,
    pub version: String,
    pub description: String,
    pub author: String,
    pub match_patterns: Vec<String>,
    pub grants: Vec<String>,
    pub run_at: String,
    pub category: String,
    pub enabled: bool,
    pub file_size: u64,
    pub modified_at: i64,
}

fn command_error(code: &str, message: impl Into<String>) -> CommandError {
    CommandError {
        code: code.to_string(),
        message: message.into(),
        detail: None,
        retryable: false,
    }
}

fn io_error(err: std::io::Error) -> CommandError {
    CommandError {
        code: "IO_ERROR".to_string(),
        message: err.to_string(),
        detail: Some(format!("{err:?}")),
        retryable: false,
    }
}

fn ensure_safe_js_file_name(file_name: &str) -> CommandResult<()> {
    if file_name.trim().is_empty()
        || file_name.contains("..")
        || file_name
            .chars()
            .any(|ch| matches!(ch, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|'))
        || !file_name.ends_with(".js")
    {
        return Err(command_error(
            "IO_ERROR",
            format!("非法扩展文件名: {file_name}"),
        ));
    }
    Ok(())
}

fn extension_dir(state: &AppState) -> std::path::PathBuf {
    state.core.reader_dir().join("extensions")
}

async fn ensure_extension_dir(state: &AppState) -> CommandResult<std::path::PathBuf> {
    let dir = extension_dir(state);
    fs::create_dir_all(&dir).await.map_err(io_error)?;
    Ok(dir)
}

fn read_meta_values(content: &str, key: &str) -> Vec<String> {
    content
        .lines()
        .take(100)
        .filter_map(|line| {
            let line = line
                .trim_start()
                .trim_start_matches("//")
                .trim_start_matches('*')
                .trim();
            if line.starts_with(key) {
                Some(
                    line[key.len()..]
                        .trim()
                        .trim_matches('"')
                        .trim_matches('\'')
                        .to_string(),
                )
            } else {
                None
            }
        })
        .filter(|value| !value.is_empty())
        .collect()
}

fn first_meta_value(content: &str, key: &str) -> Option<String> {
    read_meta_values(content, key).into_iter().next()
}

fn set_meta_enabled(content: &str, enabled: bool) -> String {
    let mut found = false;
    let mut lines = Vec::new();
    for line in content.lines() {
        let normalized = line.trim_start().trim_start_matches("//").trim_start();
        if normalized.starts_with("@enabled") {
            found = true;
            lines.push(format!("// @enabled      {enabled}"));
        } else {
            lines.push(line.to_string());
        }
    }
    if !found {
        lines.insert(0, format!("// @enabled      {enabled}"));
    }
    let mut out = lines.join("\n");
    out.push('\n');
    out
}

fn parse_extension_meta(
    file_name: String,
    content: &str,
    metadata: &std::fs::Metadata,
) -> ExtensionMeta {
    let file_size = metadata.len();
    let modified_at = metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or(0);
    let fallback_name = file_name.trim_end_matches(".js").to_string();
    let match_patterns = {
        let mut values = read_meta_values(content, "@match");
        values.extend(read_meta_values(content, "@include"));
        if values.is_empty() {
            values.push("*".to_string());
        }
        values
    };
    let grants = read_meta_values(content, "@grant")
        .into_iter()
        .filter(|value| value != "none")
        .collect();
    let enabled = first_meta_value(content, "@enabled")
        .map(|value| value != "false")
        .unwrap_or(true);

    ExtensionMeta {
        file_name,
        name: first_meta_value(content, "@name").unwrap_or(fallback_name),
        namespace: first_meta_value(content, "@namespace")
            .unwrap_or_else(|| "com.legado.extensions".to_string()),
        version: first_meta_value(content, "@version").unwrap_or_else(|| "0.0.0".to_string()),
        description: first_meta_value(content, "@description").unwrap_or_default(),
        author: first_meta_value(content, "@author").unwrap_or_default(),
        match_patterns,
        grants,
        run_at: first_meta_value(content, "@run-at").unwrap_or_else(|| "document-idle".to_string()),
        category: first_meta_value(content, "@category").unwrap_or_else(|| "其他".to_string()),
        enabled,
        file_size,
        modified_at,
    }
}

#[tauri::command]
pub async fn extension_get_dir(state: State<'_, AppState>) -> CommandResult<String> {
    let dir = ensure_extension_dir(&state).await?;
    Ok(dir.to_string_lossy().to_string())
}

#[tauri::command]
pub async fn extension_list(state: State<'_, AppState>) -> CommandResult<Vec<ExtensionMeta>> {
    let dir = ensure_extension_dir(&state).await?;
    let mut out = Vec::new();
    let mut entries = fs::read_dir(dir).await.map_err(io_error)?;
    while let Some(entry) = entries.next_entry().await.map_err(io_error)? {
        let path = entry.path();
        if path.extension().and_then(|value| value.to_str()) != Some("js") {
            continue;
        }
        let file_name = entry.file_name().to_string_lossy().to_string();
        let content = fs::read_to_string(&path).await.unwrap_or_default();
        if let Ok(metadata) = entry.metadata().await {
            out.push(parse_extension_meta(file_name, &content, &metadata));
        }
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

#[tauri::command]
pub async fn extension_read(
    state: State<'_, AppState>,
    file_name: String,
) -> CommandResult<String> {
    ensure_safe_js_file_name(&file_name)?;
    let dir = ensure_extension_dir(&state).await?;
    fs::read_to_string(dir.join(file_name))
        .await
        .map_err(io_error)
}

#[tauri::command]
pub async fn extension_save(
    state: State<'_, AppState>,
    file_name: String,
    content: String,
) -> CommandResult<()> {
    ensure_safe_js_file_name(&file_name)?;
    let dir = ensure_extension_dir(&state).await?;
    fs::write(dir.join(file_name), content)
        .await
        .map_err(io_error)
}

#[tauri::command]
pub async fn extension_delete(state: State<'_, AppState>, file_name: String) -> CommandResult<()> {
    ensure_safe_js_file_name(&file_name)?;
    let dir = ensure_extension_dir(&state).await?;
    match fs::remove_file(dir.join(file_name)).await {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(io_error(err)),
    }
}

#[tauri::command]
pub async fn extension_toggle(
    state: State<'_, AppState>,
    file_name: String,
    enabled: bool,
) -> CommandResult<()> {
    ensure_safe_js_file_name(&file_name)?;
    let dir = ensure_extension_dir(&state).await?;
    let path = dir.join(file_name);
    let content = fs::read_to_string(&path).await.map_err(io_error)?;
    fs::write(path, set_meta_enabled(&content, enabled))
        .await
        .map_err(io_error)
}

#[tauri::command]
pub async fn extension_open_in_vscode(
    state: State<'_, AppState>,
    file_name: String,
) -> CommandResult<()> {
    ensure_safe_js_file_name(&file_name)?;
    let dir = ensure_extension_dir(&state).await?;
    let path = dir.join(file_name);
    tauri_plugin_opener::open_path(path.to_string_lossy().to_string(), Some("code"))
        .or_else(|_| {
            tauri_plugin_opener::open_path(path.to_string_lossy().to_string(), None::<&str>)
        })
        .map_err(|err| command_error("IO_ERROR", err.to_string()))
}
