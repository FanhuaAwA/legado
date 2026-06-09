use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SourceRuntimeKind {
    JsScript,
    LegadoRule,
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
