use crate::{ReaderCore, ReaderCoreError};
use base64::Engine as _;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::path::Path;
use tokio::fs;

const BACKUP_FORMAT: &str = "legado-backup-v1";
const APP_CONFIG_SCOPE: &str = "app.config";
const SOURCE_DIRS_CONFIG_SCOPE: &str = "booksource.dirs";

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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SourceTextBackup {
    file_name: String,
    content: String,
}

impl ReaderCore {
    pub async fn backup_inspect(&self) -> Result<BackupInspectReport, ReaderCoreError> {
        let stats = self.backup_stats().await?;
        let total = stats.iter().map(|item| item.byte_size).sum();
        Ok(BackupInspectReport {
            categories: stats,
            total_bytes: total,
        })
    }

    pub async fn backup_create_json(
        &self,
        categories: &[String],
    ) -> Result<(String, Vec<BackupCategoryStat>), ReaderCoreError> {
        let all = self.backup_stats().await?;
        let selected = filter_stats(&all, categories);
        let payload = self.pack_backup_categories(&selected).await?;
        let raw = serde_json::to_string_pretty(&payload)?;
        Ok((raw, selected))
    }

    pub async fn backup_create_data(
        &self,
        default_name: &str,
        categories: &[String],
    ) -> Result<BackupCreateDataResult, ReaderCoreError> {
        let (raw, selected) = self.backup_create_json(categories).await?;
        Ok(BackupCreateDataResult {
            file_name: default_name.to_string(),
            mime: "application/json".to_string(),
            base64: base64::engine::general_purpose::STANDARD.encode(raw.as_bytes()),
            byte_size: raw.len() as i64,
            categories: selected,
        })
    }

    pub fn backup_peek_json(raw: &str) -> Result<BackupPeekReport, ReaderCoreError> {
        parse_backup_peek(raw)
    }

    pub fn backup_peek_data(base64: &str) -> Result<BackupPeekReport, ReaderCoreError> {
        let raw = decode_backup_base64(base64)?;
        parse_backup_peek(&raw)
    }

    pub async fn backup_restore_json(
        &self,
        raw: &str,
        categories: &[String],
    ) -> Result<BackupRestoreResult, ReaderCoreError> {
        self.restore_backup_payload(raw, categories).await
    }

    pub async fn backup_restore_data(
        &self,
        base64: &str,
        categories: &[String],
    ) -> Result<BackupRestoreResult, ReaderCoreError> {
        let raw = decode_backup_base64(base64)?;
        self.restore_backup_payload(&raw, categories).await
    }

    async fn backup_stats(&self) -> Result<Vec<BackupCategoryStat>, ReaderCoreError> {
        let mut stats = Vec::new();
        for (id, _, _) in LABELS {
            let (count, bytes) = match *id {
                "app_settings" => {
                    let value = self.config_scope_object(APP_CONFIG_SCOPE).await?;
                    (object_len(&value), json_size(&value))
                }
                "script_config" => {
                    let value = self.script_config_object().await?;
                    (nested_object_len(&value), json_size(&value))
                }
                "other_frontend" => {
                    let value = self.frontend_storage_object().await?;
                    (nested_object_len(&value), json_size(&value))
                }
                "bookshelf" => {
                    let count = self
                        .shelf_list()
                        .await
                        .map(|items| items.len())
                        .unwrap_or(0);
                    let shelf = self
                        .reader_dir()
                        .join("data")
                        .join("local")
                        .join("shelf.json");
                    let bytes = std::fs::metadata(&shelf)
                        .map(|meta| meta.len() as i64)
                        .unwrap_or(0);
                    (count as i64, bytes)
                }
                "booksources" => {
                    let source_dir = self.reader_dir().join("sources");
                    (
                        count_files(&source_dir).unwrap_or(0) as i64,
                        count_dir_size(&source_dir).unwrap_or(0) as i64,
                    )
                }
                "bookshelf_cache" => {
                    let cache = self.reader_dir().join("data").join("content");
                    (
                        count_files(&cache).unwrap_or(0) as i64,
                        count_dir_size(&cache).unwrap_or(0) as i64,
                    )
                }
                "extensions" => {
                    let dir = self.reader_dir().join("extensions");
                    (
                        count_files(&dir).unwrap_or(0) as i64,
                        count_dir_size(&dir).unwrap_or(0) as i64,
                    )
                }
                "user_fonts" => {
                    let dir = self.reader_dir().join("data").join("fonts");
                    (
                        count_files(&dir).unwrap_or(0) as i64,
                        count_dir_size(&dir).unwrap_or(0) as i64,
                    )
                }
                "reader_settings" | "sync_state" => (0, 0),
                _ => (0, 0),
            };
            stats.push(category_stat(id, count, bytes));
        }
        Ok(stats)
    }

    async fn pack_backup_categories(
        &self,
        selected: &[BackupCategoryStat],
    ) -> Result<Value, ReaderCoreError> {
        let mut data = Map::new();
        for item in selected {
            match item.id.as_str() {
                "app_settings" => {
                    data.insert(
                        item.id.clone(),
                        Value::Object(self.config_scope_object(APP_CONFIG_SCOPE).await?),
                    );
                }
                "script_config" => {
                    data.insert(
                        item.id.clone(),
                        Value::Object(self.script_config_object().await?),
                    );
                }
                "other_frontend" => {
                    data.insert(
                        item.id.clone(),
                        Value::Object(self.frontend_storage_object().await?),
                    );
                }
                "bookshelf" => {
                    if let Some(value) = self
                        .read_json_file_value(
                            &self
                                .reader_dir()
                                .join("data")
                                .join("local")
                                .join("shelf.json"),
                        )
                        .await?
                    {
                        data.insert(item.id.clone(), value);
                    }
                }
                "booksources" => {
                    data.insert(item.id.clone(), self.booksources_backup_object().await?);
                }
                _ => {}
            }
        }
        Ok(json!({
            "manifest": build_manifest(selected),
            "data": Value::Object(data),
        }))
    }

    async fn config_scope_object(
        &self,
        scope: &str,
    ) -> Result<Map<String, Value>, ReaderCoreError> {
        let raw = self.config_read_all(scope).await?;
        json_object_from_raw(&raw)
    }

    async fn script_config_object(&self) -> Result<Map<String, Value>, ReaderCoreError> {
        let mut out = Map::new();
        for namespace in self.config_list_scopes().await? {
            let Some(scope) = namespace.strip_prefix("config:") else {
                continue;
            };
            if scope == APP_CONFIG_SCOPE || scope == SOURCE_DIRS_CONFIG_SCOPE {
                continue;
            }
            let value = Value::Object(self.config_scope_object(scope).await?);
            if !value.as_object().map(Map::is_empty).unwrap_or(true) {
                out.insert(scope.to_string(), value);
            }
        }
        Ok(out)
    }

    async fn frontend_storage_object(&self) -> Result<Map<String, Value>, ReaderCoreError> {
        let mut out = Map::new();
        for summary in self.frontend_storage_list_namespaces().await? {
            let mut entries = Map::new();
            for entry in self.frontend_storage_list(&summary.namespace).await? {
                entries.insert(entry.key, Value::String(entry.value));
            }
            if !entries.is_empty() {
                out.insert(summary.namespace, Value::Object(entries));
            }
        }
        Ok(out)
    }

    async fn booksources_backup_object(&self) -> Result<Value, ReaderCoreError> {
        let source_root = self.reader_dir().join("sources");
        let legado = read_json_values_from_dir(&source_root.join("legado-json")).await?;
        let article = read_json_values_from_dir(&source_root.join("article-json")).await?;
        let scripts = read_text_backups_from_dir(&source_root.join("script-js")).await?;
        let source_dirs = self
            .config_read_json(SOURCE_DIRS_CONFIG_SCOPE, "external")
            .await?
            .unwrap_or(Value::Array(Vec::new()));
        Ok(json!({
            "legacyJson": legado,
            "articleJson": article,
            "scriptJs": scripts,
            "sourceDirs": source_dirs,
        }))
    }

    async fn read_json_file_value(&self, path: &Path) -> Result<Option<Value>, ReaderCoreError> {
        if !path.exists() {
            return Ok(None);
        }
        let raw = fs::read_to_string(path).await?;
        Ok(Some(serde_json::from_str(&raw)?))
    }

    async fn restore_backup_payload(
        &self,
        raw: &str,
        categories: &[String],
    ) -> Result<BackupRestoreResult, ReaderCoreError> {
        let payload: Value = serde_json::from_str(raw)
            .map_err(|err| ReaderCoreError::Message(format!("备份文件格式无效: {err}")))?;
        let data = payload
            .get("data")
            .ok_or_else(|| ReaderCoreError::Message("备份文件缺少 data 字段".to_string()))?;
        let filter = restore_filter(categories);
        let mut restored = Vec::new();
        let mut skipped = Vec::new();

        for category in filter {
            match category.as_str() {
                "app_settings" => {
                    if let Some(map) = data.get("app_settings").and_then(Value::as_object) {
                        let bytes = self
                            .restore_config_scope(APP_CONFIG_SCOPE, map, true)
                            .await?;
                        restored.push(category_stat(&category, map.len() as i64, bytes));
                    }
                }
                "script_config" => {
                    if let Some(scopes) = data.get("script_config").and_then(Value::as_object) {
                        let mut count = 0i64;
                        let mut bytes = 0i64;
                        for (scope, value) in scopes {
                            if let Some(map) = value.as_object() {
                                count += map.len() as i64;
                                bytes += self.restore_config_scope(scope, map, false).await?;
                            }
                        }
                        restored.push(category_stat(&category, count, bytes));
                    }
                }
                "other_frontend" => {
                    if let Some(namespaces) = data.get("other_frontend").and_then(Value::as_object)
                    {
                        let mut count = 0i64;
                        let mut bytes = 0i64;
                        for (namespace, value) in namespaces {
                            if let Some(map) = value.as_object() {
                                for (key, stored) in map {
                                    let text = match stored {
                                        Value::String(text) => text.clone(),
                                        other => other.to_string(),
                                    };
                                    bytes += text.len() as i64;
                                    count += 1;
                                    self.frontend_storage_set(namespace, key, &text).await?;
                                }
                            }
                        }
                        restored.push(category_stat(&category, count, bytes));
                    }
                }
                "bookshelf" => {
                    if let Some(shelf) = data.get("bookshelf") {
                        let path = self
                            .reader_dir()
                            .join("data")
                            .join("local")
                            .join("shelf.json");
                        if let Some(parent) = path.parent() {
                            fs::create_dir_all(parent).await?;
                        }
                        let raw = serde_json::to_string_pretty(shelf)?;
                        fs::write(&path, &raw).await?;
                        restored.push(category_stat(&category, 1, raw.len() as i64));
                    }
                }
                "booksources" => {
                    if let Some(value) = data.get("booksources") {
                        let stat = self.restore_booksources(value).await?;
                        restored.push(category_stat(&category, stat.0, stat.1));
                    }
                }
                _ => skipped.push(category),
            }
        }
        Ok(BackupRestoreResult { restored, skipped })
    }

    async fn restore_config_scope(
        &self,
        scope: &str,
        map: &Map<String, Value>,
        app_config: bool,
    ) -> Result<i64, ReaderCoreError> {
        let mut bytes = 0i64;
        for (key, value) in map {
            bytes += serde_json::to_string(value)?.len() as i64;
            if app_config {
                self.app_config_set(key, value).await?;
            } else {
                self.config_write_json(scope, key, value).await?;
            }
        }
        Ok(bytes)
    }

    async fn restore_booksources(&self, value: &Value) -> Result<(i64, i64), ReaderCoreError> {
        let mut count = 0i64;
        let mut bytes = 0i64;
        let mut legacy_items = Vec::new();

        if let Some(items) = value.as_array() {
            for (idx, item) in items.iter().enumerate() {
                let raw = serde_json::to_string_pretty(item)?;
                bytes += raw.len() as i64;
                legacy_items.push((format!("booksources-{idx}.json"), raw));
            }
        } else if let Some(object) = value.as_object() {
            for key in ["legacyJson", "articleJson"] {
                if let Some(items) = object.get(key).and_then(Value::as_array) {
                    for (idx, item) in items.iter().enumerate() {
                        let raw = serde_json::to_string_pretty(item)?;
                        bytes += raw.len() as i64;
                        legacy_items.push((format!("{key}-{idx}.json"), raw));
                    }
                }
            }
            if let Some(scripts) = object.get("scriptJs").and_then(Value::as_array) {
                for script in scripts {
                    let Some(file_name) = script.get("fileName").and_then(Value::as_str) else {
                        continue;
                    };
                    let Some(content) = script.get("content").and_then(Value::as_str) else {
                        continue;
                    };
                    if !is_safe_backup_file_name(file_name) {
                        continue;
                    }
                    self.save_js_source(file_name, content, None).await?;
                    count += 1;
                    bytes += content.len() as i64;
                }
            }
        }

        if !legacy_items.is_empty() {
            let result = self.import_legacy_json_texts(&legacy_items, false).await?;
            count += result.imported as i64;
        }
        Ok((count, bytes))
    }
}

fn build_manifest(stats: &[BackupCategoryStat]) -> BackupManifest {
    BackupManifest {
        format: BACKUP_FORMAT.to_string(),
        version: 1,
        created_at: chrono::Utc::now().timestamp_millis(),
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        categories: stats.to_vec(),
    }
}

fn parse_backup_peek(raw: &str) -> Result<BackupPeekReport, ReaderCoreError> {
    let payload: Value = serde_json::from_str(raw)
        .map_err(|err| ReaderCoreError::Message(format!("备份文件格式无效: {err}")))?;
    let manifest: BackupManifest = serde_json::from_value(
        payload
            .get("manifest")
            .cloned()
            .ok_or_else(|| ReaderCoreError::Message("备份文件缺少 manifest 字段".to_string()))?,
    )?;
    let known: Vec<String> = LABELS.iter().map(|(id, _, _)| id.to_string()).collect();
    let unknown = manifest
        .categories
        .iter()
        .map(|item| item.id.clone())
        .filter(|id| !known.contains(id))
        .collect();
    Ok(BackupPeekReport {
        manifest,
        unknown_categories: unknown,
    })
}

fn decode_backup_base64(input: &str) -> Result<String, ReaderCoreError> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(input)
        .map_err(|err| ReaderCoreError::Message(err.to_string()))?;
    String::from_utf8(bytes).map_err(|err| ReaderCoreError::Message(err.to_string()))
}

fn filter_stats(stats: &[BackupCategoryStat], categories: &[String]) -> Vec<BackupCategoryStat> {
    if categories.is_empty() || categories.iter().any(|item| item == "all") {
        return stats.to_vec();
    }
    stats
        .iter()
        .filter(|item| categories.contains(&item.id))
        .cloned()
        .collect()
}

fn restore_filter(categories: &[String]) -> Vec<String> {
    if categories.is_empty() || categories.iter().any(|item| item == "all") {
        return LABELS.iter().map(|(id, _, _)| id.to_string()).collect();
    }
    categories.to_vec()
}

fn category_stat(id: &str, item_count: i64, byte_size: i64) -> BackupCategoryStat {
    let (label, description) = LABELS
        .iter()
        .find(|(candidate, _, _)| *candidate == id)
        .map(|(_, label, description)| (*label, *description))
        .unwrap_or((id, ""));
    BackupCategoryStat {
        id: id.to_string(),
        label: label.to_string(),
        description: description.to_string(),
        item_count,
        byte_size,
    }
}

fn json_object_from_raw(raw: &str) -> Result<Map<String, Value>, ReaderCoreError> {
    match serde_json::from_str::<Value>(raw)? {
        Value::Object(map) => Ok(map),
        _ => Ok(Map::new()),
    }
}

fn object_len(map: &Map<String, Value>) -> i64 {
    map.len() as i64
}

fn nested_object_len(map: &Map<String, Value>) -> i64 {
    map.values()
        .filter_map(Value::as_object)
        .map(|value| value.len() as i64)
        .sum()
}

fn json_size(map: &Map<String, Value>) -> i64 {
    serde_json::to_string(map)
        .map(|raw| raw.len() as i64)
        .unwrap_or(0)
}

fn count_dir_size(path: &Path) -> std::io::Result<u64> {
    if !path.exists() {
        return Ok(0);
    }
    let mut total = 0u64;
    for entry in std::fs::read_dir(path)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() {
            total += entry.metadata()?.len();
        } else if path.is_dir() {
            total += count_dir_size(&path)?;
        }
    }
    Ok(total)
}

fn count_files(path: &Path) -> std::io::Result<usize> {
    if !path.exists() {
        return Ok(0);
    }
    let mut count = 0usize;
    for entry in std::fs::read_dir(path)? {
        let path = entry?.path();
        if path.is_file() {
            count += 1;
        } else if path.is_dir() {
            count += count_files(&path)?;
        }
    }
    Ok(count)
}

async fn read_json_values_from_dir(path: &Path) -> Result<Vec<Value>, ReaderCoreError> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    let mut entries = fs::read_dir(path).await?;
    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        if !path.is_file() || path.extension().and_then(|value| value.to_str()) != Some("json") {
            continue;
        }
        let raw = fs::read_to_string(path).await?;
        if let Ok(value) = serde_json::from_str::<Value>(&raw) {
            out.push(value);
        }
    }
    Ok(out)
}

async fn read_text_backups_from_dir(path: &Path) -> Result<Vec<SourceTextBackup>, ReaderCoreError> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    let mut entries = fs::read_dir(path).await?;
    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Some(file_name) = path.file_name().and_then(|value| value.to_str()) else {
            continue;
        };
        let content = fs::read_to_string(&path).await?;
        out.push(SourceTextBackup {
            file_name: file_name.to_string(),
            content,
        });
    }
    Ok(out)
}

fn is_safe_backup_file_name(value: &str) -> bool {
    !value.is_empty()
        && !value.contains("..")
        && !value.contains(['/', '\\', ':', '?', '*', '"', '<', '>', '|'])
}
