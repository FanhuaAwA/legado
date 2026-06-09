use crate::state::AppState;
use reader_core::CommandError;
use serde::{Deserialize, Serialize};
use tauri::State;

type CommandResult<T> = Result<T, CommandError>;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SysFont {
    pub name: String,
    pub cjk_likely: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserFontMeta {
    pub id: String,
    pub file_path: String,
    pub family_name: String,
    pub display_name: String,
    pub uploaded_at: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UploadFontRequest {
    pub file_name: String,
    pub data: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteFontRequest {
    pub id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenameFontRequest {
    pub id: String,
    pub display_name: String,
}

fn user_fonts_dir(state: &AppState) -> std::path::PathBuf {
    state.core.reader_dir().join("data").join("fonts")
}

fn user_fonts_meta_path(state: &AppState) -> std::path::PathBuf {
    user_fonts_dir(state).join("_meta.json")
}

fn load_meta(state: &AppState) -> Vec<UserFontMeta> {
    let path = user_fonts_meta_path(state);
    if let Ok(raw) = std::fs::read_to_string(&path) {
        serde_json::from_str(&raw).unwrap_or_default()
    } else {
        Vec::new()
    }
}

fn save_meta(state: &AppState, meta: &[UserFontMeta]) {
    let dir = user_fonts_dir(state);
    let _ = std::fs::create_dir_all(&dir);
    if let Ok(json) = serde_json::to_string_pretty(meta) {
        let _ = std::fs::write(user_fonts_meta_path(state), &json);
    }
}

#[tauri::command]
pub fn list_system_fonts() -> CommandResult<Vec<SysFont>> {
    let mut fonts = Vec::new();
    let font_dirs = [
        "C:\\Windows\\Fonts",
        "/usr/share/fonts",
        "/usr/local/share/fonts",
        "/System/Library/Fonts",
    ];
    let cjk_ranges = [
        ('\u{4E00}'..='\u{9FFF}'),   // CJK Unified
        ('\u{3400}'..='\u{4DBF}'),   // CJK Ext-A
        ('\u{AC00}'..='\u{D7AF}'),   // Hangul
        ('\u{3040}'..='\u{309F}'),   // Hiragana
        ('\u{30A0}'..='\u{30FF}'),   // Katakana
    ];

    for dir in &font_dirs {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
                if matches!(ext.as_str(), "ttf" | "otf" | "ttc" | "fon" | "woff" | "woff2") {
                    let name = path.file_stem().unwrap_or_default().to_string_lossy().to_string();
                    // Quick CJK check: if the name contains any CJK character
                    let cjk = name.chars().any(|c| cjk_ranges.iter().any(|r| r.contains(&c)));
                    fonts.push(SysFont { name, cjk_likely: cjk });
                }
            }
        }
        if !fonts.is_empty() { break; }
    }

    // Also try DWrite API on Windows for better coverage
    #[cfg(target_os = "windows")]
    {
        use std::process::Command;
        if let Ok(output) = Command::new("powershell")
            .args([
                "-NoProfile", "-Command",
                "[System.Reflection.Assembly]::LoadWithPartialName('System.Drawing'); (New-Object System.Drawing.Text.InstalledFontCollection).Families | ForEach-Object { $_.Name }",
            ])
            .output()
        {
            if output.status.success() {
                let text = String::from_utf8_lossy(&output.stdout);
                for line in text.lines() {
                    let name = line.trim().to_string();
                    if !name.is_empty() && !fonts.iter().any(|f| f.name == name) {
                        let cjk = name.chars().any(|c| cjk_ranges.iter().any(|r| r.contains(&c)));
                        fonts.push(SysFont { name, cjk_likely: cjk });
                    }
                }
            }
        }
    }

    fonts.sort_by(|a, b| a.name.cmp(&b.name));
    fonts.dedup_by(|a, b| a.name == b.name);
    Ok(fonts)
}

#[tauri::command]
pub fn list_user_fonts(state: State<'_, AppState>) -> CommandResult<Vec<UserFontMeta>> {
    Ok(load_meta(&state))
}

#[tauri::command]
pub fn upload_user_font(state: State<'_, AppState>, request: UploadFontRequest) -> CommandResult<()> {
    let dir = user_fonts_dir(&state);
    std::fs::create_dir_all(&dir).map_err(|e| CommandError {
        code: "IO_ERROR".into(), message: e.to_string(), detail: None, retryable: false,
    })?;
    use base64::Engine as _;
    let bytes = base64::engine::general_purpose::STANDARD.decode(&request.data).map_err(|e| CommandError {
        code: "IO_ERROR".into(), message: e.to_string(), detail: None, retryable: false,
    })?;
    let file_path = dir.join(&request.file_name);
    std::fs::write(&file_path, &bytes).map_err(|e| CommandError {
        code: "IO_ERROR".into(), message: e.to_string(), detail: None, retryable: false,
    })?;
    let mut meta = load_meta(&state);
    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().timestamp_millis();
    let family = request.file_name.trim_end_matches(|c: char| c == '.' || c.is_ascii_digit()).replace('.', " ").trim().to_string();
    meta.push(UserFontMeta {
        id,
        file_path: file_path.to_string_lossy().to_string(),
        family_name: family,
        display_name: request.file_name.clone(),
        uploaded_at: now,
    });
    save_meta(&state, &meta);
    Ok(())
}

#[tauri::command]
pub fn delete_user_font(state: State<'_, AppState>, request: DeleteFontRequest) -> CommandResult<()> {
    let mut meta = load_meta(&state);
    if let Some(font) = meta.iter().find(|f| f.id == request.id) {
        let path = std::path::PathBuf::from(&font.file_path);
        if path.exists() { let _ = std::fs::remove_file(&path); }
    }
    meta.retain(|f| f.id != request.id);
    save_meta(&state, &meta);
    Ok(())
}

#[tauri::command]
pub fn rename_user_font(state: State<'_, AppState>, request: RenameFontRequest) -> CommandResult<()> {
    let mut meta = load_meta(&state);
    if let Some(font) = meta.iter_mut().find(|f| f.id == request.id) {
        font.display_name = request.display_name;
        save_meta(&state, &meta);
    }
    Ok(())
}
