use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SourceRuntimeKind {
    JsScript,
    LegadoRule,
    LegacyArticle,
}

impl SourceRuntimeKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            SourceRuntimeKind::JsScript => "js",
            SourceRuntimeKind::LegadoRule => "legado",
            SourceRuntimeKind::LegacyArticle => "article",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "js" | "jsscript" => SourceRuntimeKind::JsScript,
            "legado" | "legadorule" => SourceRuntimeKind::LegadoRule,
            "article" | "legacyarticle" => SourceRuntimeKind::LegacyArticle,
            _ => SourceRuntimeKind::LegadoRule,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceRef {
    pub source_id: String,
    pub file_name: Option<String>,
    pub source_dir: Option<String>,
    pub runtime: SourceRuntimeKind,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BookSourceMeta {
    pub source_key: String,
    pub uuid: String,
    pub file_name: String,
    pub name: String,
    pub url: String,
    pub urls: Vec<String>,
    pub homepage_url: Option<String>,
    pub author: Option<String>,
    pub logo: Option<String>,
    pub description: Option<String>,
    pub enabled: bool,
    pub file_size: u64,
    pub modified_at: i64,
    pub source_dir: String,
    pub source_type: String,
    pub version: String,
    pub update_url: Option<String>,
    pub tags: Vec<String>,
    pub min_delay_ms: u64,
    pub require_urls: Vec<String>,
    pub has_explore: Option<bool>,
    pub runtime: SourceRuntimeKind,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LegacyJsonImportResult {
    pub imported: usize,
    pub skipped: usize,
    pub files: Vec<String>,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BookItem {
    pub name: String,
    pub author: String,
    pub book_url: String,
    pub cover_url: Option<String>,
    pub last_chapter: Option<String>,
    pub latest_chapter: Option<String>,
    pub latest_chapter_url: Option<String>,
    pub word_count: Option<String>,
    pub chapter_count: Option<i32>,
    pub update_time: Option<String>,
    pub status: Option<String>,
    pub kind: Option<String>,
    pub intro: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BookDetail {
    pub name: String,
    pub author: String,
    pub book_url: Option<String>,
    pub cover_url: Option<String>,
    pub intro: Option<String>,
    pub kind: Option<String>,
    pub last_chapter: Option<String>,
    pub latest_chapter: Option<String>,
    pub latest_chapter_url: Option<String>,
    pub word_count: Option<String>,
    pub chapter_count: Option<i32>,
    pub update_time: Option<String>,
    pub status: Option<String>,
    pub toc_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChapterItem {
    pub name: String,
    pub url: String,
    pub group: Option<String>,
    pub vip: Option<bool>,
    pub is_vip: Option<bool>,
    pub price: Option<Value>,
    pub currency: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShelfBook {
    pub id: String,
    pub name: String,
    pub author: String,
    pub cover_url: Option<String>,
    pub cover_referer: Option<String>,
    pub intro: Option<String>,
    pub kind: Option<String>,
    pub group_id: Option<String>,
    pub book_url: String,
    pub file_name: String,
    pub source_dir: Option<String>,
    pub source_name: String,
    pub last_chapter: Option<String>,
    pub added_at: i64,
    pub last_read_at: i64,
    pub read_chapter_index: i32,
    pub read_chapter_url: Option<String>,
    pub total_chapters: i32,
    pub source_type: String,
    pub read_page_index: i32,
    pub read_scroll_ratio: f64,
    pub read_playback_time: f64,
    pub reader_settings: Option<String>,
    pub is_private: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddBookPayload {
    pub name: String,
    pub author: Option<String>,
    pub cover_url: Option<String>,
    pub intro: Option<String>,
    pub kind: Option<String>,
    pub group_id: Option<String>,
    pub book_url: String,
    pub source_dir: Option<String>,
    pub last_chapter: Option<String>,
    pub source_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateShelfBookPayload {
    pub id: String,
    pub name: String,
    pub author: Option<String>,
    pub cover_url: Option<String>,
    pub intro: Option<String>,
    pub kind: Option<String>,
    pub group_id: Option<String>,
    pub book_url: String,
    pub file_name: String,
    pub source_dir: Option<String>,
    pub source_name: String,
    pub last_chapter: Option<String>,
    pub total_chapters: i32,
    pub read_chapter_index: i32,
    pub read_chapter_url: Option<String>,
    pub source_type: String,
    pub added_at: Option<i64>,
    pub last_read_at: Option<i64>,
    pub read_page_index: Option<i32>,
    pub read_scroll_ratio: Option<f64>,
    pub read_playback_time: Option<f64>,
    pub reader_settings: Option<String>,
    pub is_private: Option<bool>,
    pub create_source_switch_backup: Option<bool>,
    pub clear_content_cache: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CachedChapter {
    pub index: i32,
    pub name: String,
    pub url: String,
    pub group: Option<String>,
    pub vip: Option<bool>,
    pub price: Option<Value>,
    pub currency: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceSwitchRestoreResult {
    pub book: ShelfBook,
    pub chapters: Vec<CachedChapter>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EpisodeProgress {
    pub time: f64,
    pub duration: f64,
    pub last_played_at: i64,
}

pub type EpisodeProgressMap = HashMap<String, EpisodeProgress>;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FrontendStorageEntry {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FrontendStorageNamespaceSummary {
    pub namespace: String,
    pub count: usize,
}

// ── WebDAV 同步（CAP-SYNC）────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncStatus {
    pub enabled: bool,
    pub running: bool,
    pub last_success_at: i64,
    pub last_failed_at: i64,
    pub last_error: String,
    pub dirty_domains: Vec<String>,
    pub conflict_count: usize,
    pub last_run_summary: String,
}

impl Default for SyncStatus {
    fn default() -> Self {
        Self {
            enabled: false,
            running: false,
            last_success_at: 0,
            last_failed_at: 0,
            last_error: String::new(),
            dirty_domains: Vec::new(),
            conflict_count: 0,
            last_run_summary: "idle".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncCredentials {
    pub password: String,
    #[serde(default)]
    pub password_set: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncConnectionTestResult {
    pub ok: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncRunSummary {
    pub status: String,
    pub mode: String,
    pub domains: Vec<String>,
    pub uploaded_domains: Vec<String>,
    pub applied_domains: Vec<String>,
    pub conflict_count: usize,
    pub message: String,
    #[serde(default)]
    pub client_states: Vec<SyncClientState>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncClientState {
    pub domain: String,
    pub value: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncConflict {
    pub id: String,
    pub domain: String,
    pub key: String,
    pub message: String,
    pub local: Value,
    pub remote: Value,
    pub created_at: i64,
    pub resolved: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReaderSessionPayload {
    pub active: bool,
    pub book_id: String,
    pub chapter_index: i32,
    pub chapter_name: String,
    pub chapter_url: String,
    pub page_index: i32,
    pub scroll_ratio: f64,
    pub playback_time: f64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncV2ProgressResult {
    pub status: String,
    pub message: String,
    #[serde(default)]
    pub local: Option<Value>,
    #[serde(default)]
    pub remote: Option<Value>,
}

// ── 书源仓库 / 在线更新（CAP-REPO）──────────────────────────────

/// Result of checking a single JS source against its `@updateUrl`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceUpdateCheck {
    pub file_name: String,
    pub uuid: String,
    pub has_update: bool,
    pub local_version: String,
    pub remote_version: String,
}

/// A book-source repository manifest (remote JSON). Optional fields default so a
/// partial manifest still parses; the frontend reports per-field problems.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RepoManifest {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub updated_at: String,
    #[serde(default)]
    pub sources: Vec<RepoSourceInfo>,
}

/// One source entry inside a repository manifest.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RepoSourceInfo {
    #[serde(default)]
    pub uuid: Option<String>,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub logo: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub file_name: String,
    #[serde(default)]
    pub download_url: String,
    #[serde(default)]
    pub file_size: u64,
    #[serde(default)]
    pub updated_at: String,
}

fn default_true() -> bool {
    true
}

/// Preview of a remote JS source downloaded before install.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteSourcePreview {
    pub download_url: String,
    pub meta: BookSourceMeta,
    pub has_explicit_uuid: bool,
}

/// Whether a remote repository source matches the locally installed copy
/// (ignoring `@enabled` / `@uuid` lines).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoSourceSync {
    pub file_name: String,
    pub uuid: String,
    pub is_consistent: bool,
    pub local_version: String,
    pub remote_version: String,
}
