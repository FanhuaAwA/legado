use crate::app_state::ReaderCoreOptions;
use crate::crawler::http_client::{HttpClient, HttpClientConfig};
use crate::dto::{
    AddBookPayload, BookDetail, BookItem, BookSourceMeta, CachedChapter, ChapterItem,
    EpisodeProgress, EpisodeProgressMap, FrontendStorageEntry, FrontendStorageNamespaceSummary,
    LegacyJsonImportProgress, LegacyJsonImportResult, ReaderSessionPayload, RemoteSourcePreview,
    RepoManifest, RepoSourceSync, ShelfBook, SourceRuntimeKind, SourceSwitchRestoreResult,
    SourceUpdateCheck, SyncClientState, SyncConflict, SyncConnectionTestResult, SyncCredentials,
    SyncRunSummary, SyncStatus, SyncV2ProgressResult, UpdateShelfBookPayload,
};
use crate::error::ReaderCoreError;
use crate::model::ai_proxy::{ai_proxy_timeout, validate_ai_proxy_url, AiHttpProxyResponse};
use crate::model::article_source::ArticleSource;
use crate::model::book::Book;
use crate::model::book_chapter::BookChapter;
use crate::model::book_source::{
    book_source_from_value, migrate_legacy_book_source_value, BookSource,
};
use crate::model::search::SearchBook;
use crate::parser::js::{eval_js, with_js_source, JsSourceArg};
use crate::parser::rule_engine::{derive_toc_url_from_chapter_url, RuleEngine};
use crate::service::book_service::BookService;
use crate::service::book_source_service::BookSourceService;
use crate::service::json_document_service::JsonDocumentService;
use crate::service::sync_webdav::{SyncRuntime, WebDavClient, WebDavConfig};
use crate::source_runtime::js_source::JsSourceRuntime;
use crate::storage::cache::file_cache::FileCache;
use crate::storage::db::init_pool;
use crate::storage::db::repo::BookSourceListRow;
use crate::util::hash::md5_hex;
use crate::util::time::now_ts;
use serde::de::IgnoredAny;
use serde::Deserialize;
use serde::Serialize;
use serde_json::{json, Value};
use sqlx::{Row, SqlitePool};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};
use tokio::fs;

const USER_NS: &str = "local";
const LEGACY_IMPORT_PROGRESS_INTERVAL: usize = 25;
const LEGADO_SOURCE_DIR_LABEL: &str = "legado-json";
const FRONTEND_STORAGE_PREFIX: &str = "frontend:";
const APP_CONFIG_SCOPE: &str = "app.config";
const SOURCE_DIRS_CONFIG_SCOPE: &str = "booksource.dirs";
const SOURCE_DIRS_CONFIG_KEY: &str = "external";
const LEGADO_BROWSER_ACTION_FN: &str = "__legado_browser_action";
const SOURCE_LIST_CACHE_TTL: Duration = Duration::from_secs(30 * 60);
const SOURCE_LIST_DB_PAGE_SIZE: usize = 64;
const SOURCE_TEXT_CACHE_TTL: Duration = Duration::from_secs(30 * 60);
const LEGADO_SOURCE_CACHE_TTL: Duration = Duration::from_secs(30 * 60);
const SYNC_SUPPORTED_DOMAINS: &[&str] = &[
    "bookshelf",
    "reading_progress",
    "booksources",
    "app_settings",
    "reader_settings",
    "source_flags",
];
const SYNC_DEFERRED_DOMAINS: &[&str] = &["extensions", "script_config"];

#[derive(Clone)]
struct SourceListCache {
    items: Vec<BookSourceMeta>,
    loaded_at: Instant,
}

#[derive(Clone)]
struct SourceTextCacheEntry {
    content: String,
    modified: Option<SystemTime>,
    len: u64,
    loaded_at: Instant,
}

#[derive(Clone)]
struct LegadoSourceCacheEntry {
    source: BookSource,
    modified: Option<SystemTime>,
    len: u64,
    loaded_at: Instant,
}

#[derive(Clone)]
pub struct ReaderCore {
    reader_dir: PathBuf,
    js_source_dir: PathBuf,
    legado_source_dir: PathBuf,
    pool: SqlitePool,
    source_service: BookSourceService,
    book_service: BookService,
    document_service: JsonDocumentService,
    sync_runtime: Arc<SyncRuntime>,
    source_list_cache: Arc<tokio::sync::RwLock<Option<SourceListCache>>>,
    source_text_cache: Arc<tokio::sync::RwLock<HashMap<PathBuf, SourceTextCacheEntry>>>,
    legado_source_cache: Arc<tokio::sync::RwLock<HashMap<String, LegadoSourceCacheEntry>>>,
    /// 每本书在跑的缓存任务取消令牌：同书新任务自动取消旧任务，
    /// 防止前端调用超时重发导致任务堆积、对书源高频请求。
    prefetch_tasks: Arc<tokio::sync::Mutex<HashMap<String, Arc<AtomicBool>>>>,
}

impl ReaderCore {
    pub async fn new(options: ReaderCoreOptions) -> Result<Self, ReaderCoreError> {
        let reader_dir = options.app_data_dir.join("reader");
        let js_source_dir = reader_dir.join("sources").join("script-js");
        let legado_source_dir = reader_dir.join("sources").join(LEGADO_SOURCE_DIR_LABEL);
        fs::create_dir_all(&js_source_dir).await?;
        fs::create_dir_all(&legado_source_dir).await?;
        fs::create_dir_all(reader_dir.join("cache").join("chapters")).await?;
        fs::create_dir_all(reader_dir.join("config")).await?;

        let db_path = reader_dir.join("reader.db");
        let database_url = format!("sqlite:{}?mode=rwc", db_path.display());
        let pool = init_pool(&database_url).await?;
        let storage_dir = reader_dir.to_string_lossy().to_string();
        let source_service = BookSourceService::new(
            crate::storage::db::repo::BookSourceRepo::new(pool.clone()),
            &storage_dir,
        );
        let app_config = load_app_config(&pool).await;
        let http_cfg = HttpClientConfig::from_app_config(&app_config, options.request_timeout_secs);
        let http = HttpClient::from_config(&http_cfg)?;
        crate::parser::js::set_js_http_min_delay_ms(config_u64_value(
            &app_config,
            "request_min_delay_ms",
            300,
        ));
        // Keep the JS HTTP bridge's TLS policy in sync with the main client so
        // the Network panel's "ignore TLS errors" toggle governs every path.
        crate::parser::js::set_js_http_ignore_tls(http_cfg.ignore_tls_errors);
        // Same for DoH: the bridge honors http_doh_server like the main client.
        crate::parser::js::set_js_http_doh_server(&http_cfg.doh_server);
        crate::parser::js::set_js_engine_timeout_secs(config_u64_value(
            &app_config,
            "engine_timeout_secs",
            30,
        ));
        let book_service = BookService::new(
            http,
            RuleEngine::new()?,
            FileCache::new(reader_dir.join("cache").join("chapters")),
            &storage_dir,
        );
        let document_service = JsonDocumentService::new(pool.clone(), &storage_dir);

        Ok(Self {
            reader_dir,
            js_source_dir,
            legado_source_dir,
            pool,
            source_service,
            book_service,
            document_service,
            sync_runtime: Arc::new(SyncRuntime::new()),
            source_list_cache: Arc::new(tokio::sync::RwLock::new(None)),
            source_text_cache: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
            legado_source_cache: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
            prefetch_tasks: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
        })
    }

    pub fn reader_dir(&self) -> &Path {
        &self.reader_dir
    }

    pub fn js_source_dir(&self) -> &Path {
        &self.js_source_dir
    }

    pub async fn source_dirs(&self) -> Result<Vec<String>, ReaderCoreError> {
        let mut dirs = vec![
            self.js_source_dir.to_string_lossy().to_string(),
            self.legado_source_dir.to_string_lossy().to_string(),
        ];
        dirs.extend(self.external_source_dirs().await?);
        dedupe_strings(&mut dirs);
        Ok(dirs)
    }

    pub async fn add_source_dir(&self, dir_path: &str) -> Result<(), ReaderCoreError> {
        let path = normalize_source_dir(dir_path)?;
        let metadata = fs::metadata(&path).await?;
        if !metadata.is_dir() {
            return Err(ReaderCoreError::Message(format!(
                "书源目录不是文件夹: {}",
                path.display()
            )));
        }

        let mut dirs = self.external_source_dirs().await?;
        dirs.push(path.to_string_lossy().to_string());
        dedupe_strings(&mut dirs);
        self.save_external_source_dirs(&dirs).await?;
        self.clear_source_text_cache().await;
        self.invalidate_source_list_cache().await;
        Ok(())
    }

    pub async fn remove_source_dir(&self, dir_path: &str) -> Result<(), ReaderCoreError> {
        let path = normalize_source_dir(dir_path)?;
        let target = path.to_string_lossy().to_string();
        let mut dirs = self.external_source_dirs().await?;
        dirs.retain(|dir| dir != &target);
        self.save_external_source_dirs(&dirs).await?;
        self.clear_source_text_cache().await;
        self.invalidate_source_list_cache().await;
        Ok(())
    }

    pub async fn list_sources(&self) -> Result<Vec<BookSourceMeta>, ReaderCoreError> {
        if let Some(items) = self.cached_source_list(false).await {
            return Ok(items);
        }
        let out = self.scan_sources().await?;
        self.replace_source_list_cache(out.clone()).await;
        Ok(out)
    }

    pub async fn stream_sources<F, Fut>(
        &self,
        batch_size: usize,
        force: bool,
        mut emit: F,
    ) -> Result<usize, ReaderCoreError>
    where
        F: FnMut(Vec<BookSourceMeta>, bool, Option<usize>) -> Fut,
        Fut: Future<Output = ()>,
    {
        let batch_size = batch_size.max(1);
        if let Some(items) = self.cached_source_list(force).await {
            let total = items.len();
            if total == 0 {
                emit(Vec::new(), true, Some(0)).await;
                return Ok(0);
            }
            for (idx, chunk) in items.chunks(batch_size).enumerate() {
                let done = (idx + 1) * batch_size >= total;
                emit(chunk.to_vec(), done, Some(total)).await;
            }
            return Ok(total);
        }

        let mut all = Vec::new();
        let mut batch = Vec::with_capacity(batch_size);
        self.stream_legado_sources(batch_size, &mut all, &mut batch, &mut emit)
            .await?;
        self.stream_js_sources(batch_size, &mut all, &mut batch, &mut emit)
            .await?;
        self.stream_article_sources(batch_size, &mut all, &mut batch, &mut emit)
            .await?;

        let total = all.len() + batch.len();
        all.extend(batch.iter().cloned());
        all.sort_by(|a, b| a.name.cmp(&b.name));
        if batch.is_empty() {
            emit(Vec::new(), true, Some(total)).await;
        } else {
            let final_batch = std::mem::take(&mut batch);
            emit(final_batch, true, Some(total)).await;
        }
        self.replace_source_list_cache(all).await;
        Ok(total)
    }

    async fn scan_sources(&self) -> Result<Vec<BookSourceMeta>, ReaderCoreError> {
        let mut out = Vec::new();
        out.extend(self.list_js_sources().await?);
        out.extend(self.list_legado_sources().await?);
        out.extend(self.list_article_sources().await?);
        out.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(out)
    }

    async fn cached_source_list(&self, force: bool) -> Option<Vec<BookSourceMeta>> {
        if force {
            return None;
        }
        self.source_list_cache
            .read()
            .await
            .as_ref()
            .filter(|cache| cache.loaded_at.elapsed() < SOURCE_LIST_CACHE_TTL)
            .map(|cache| cache.items.clone())
    }

    async fn replace_source_list_cache(&self, items: Vec<BookSourceMeta>) {
        *self.source_list_cache.write().await = Some(SourceListCache {
            items,
            loaded_at: Instant::now(),
        });
    }

    async fn invalidate_source_list_cache(&self) {
        *self.source_list_cache.write().await = None;
    }

    async fn read_source_text_cached(&self, path: &Path) -> Result<String, ReaderCoreError> {
        let metadata = fs::metadata(path).await?;
        if let Some(content) = self.cached_source_text(path, &metadata).await {
            return Ok(content);
        }

        let content = fs::read_to_string(path).await?;
        self.store_source_text_cache(path, &content, &metadata)
            .await;
        Ok(content)
    }

    async fn cached_source_text(
        &self,
        path: &Path,
        metadata: &std::fs::Metadata,
    ) -> Option<String> {
        let modified = metadata.modified().ok();
        self.source_text_cache
            .read()
            .await
            .get(path)
            .filter(|entry| {
                entry.loaded_at.elapsed() < SOURCE_TEXT_CACHE_TTL
                    && entry.len == metadata.len()
                    && entry.modified == modified
            })
            .map(|entry| entry.content.clone())
    }

    async fn store_source_text_cache(
        &self,
        path: &Path,
        content: &str,
        metadata: &std::fs::Metadata,
    ) {
        self.source_text_cache.write().await.insert(
            path.to_path_buf(),
            SourceTextCacheEntry {
                content: content.to_string(),
                modified: metadata.modified().ok(),
                len: metadata.len(),
                loaded_at: Instant::now(),
            },
        );
    }

    async fn remove_source_text_cache(&self, path: &Path) {
        self.source_text_cache.write().await.remove(path);
    }

    async fn clear_source_text_cache(&self) {
        self.source_text_cache.write().await.clear();
    }

    async fn cached_legado_source(
        &self,
        file_name: &str,
        metadata: Option<&std::fs::Metadata>,
    ) -> Option<BookSource> {
        let modified = metadata.and_then(|value| value.modified().ok());
        let len = metadata.map(|value| value.len()).unwrap_or(0);
        self.legado_source_cache
            .read()
            .await
            .get(file_name)
            .filter(|entry| {
                entry.loaded_at.elapsed() < LEGADO_SOURCE_CACHE_TTL
                    && entry.len == len
                    && entry.modified == modified
            })
            .map(|entry| entry.source.clone())
    }

    async fn store_legado_source_cache(
        &self,
        file_name: &str,
        source: &BookSource,
        metadata: Option<&std::fs::Metadata>,
    ) {
        self.legado_source_cache.write().await.insert(
            file_name.to_string(),
            LegadoSourceCacheEntry {
                source: source.clone(),
                modified: metadata.and_then(|value| value.modified().ok()),
                len: metadata.map(|value| value.len()).unwrap_or(0),
                loaded_at: Instant::now(),
            },
        );
    }

    async fn remove_legado_source_cache(&self, file_name: &str) {
        self.legado_source_cache.write().await.remove(file_name);
    }

    async fn list_article_sources(&self) -> Result<Vec<BookSourceMeta>, ReaderCoreError> {
        let article_dir = self.reader_dir.join("sources").join("article-json");
        let mut out = Vec::new();
        let mut entries = match fs::read_dir(&article_dir).await {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(out),
            Err(e) => return Err(e.into()),
        };
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().and_then(|v| v.to_str()) != Some("json") {
                continue;
            }
            let file_name = entry.file_name().to_string_lossy().to_string();
            let content = fs::read_to_string(&path).await.unwrap_or_default();
            if let Ok(article) = serde_json::from_str::<ArticleSource>(&content) {
                let metadata = fs::metadata(&path).await.ok();
                let modified = metadata
                    .as_ref()
                    .and_then(|m| {
                        m.modified()
                            .ok()
                            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                            .map(|d| d.as_secs() as i64)
                    })
                    .unwrap_or(0);
                let size = metadata.as_ref().map(|m| m.len()).unwrap_or(0);
                out.push(BookSourceMeta {
                    source_key: format!("article:{}", article.source_name),
                    uuid: article.source_name.clone(),
                    file_name,
                    name: article.source_name,
                    url: article.source_url.clone(),
                    urls: vec![article.source_url.clone()],
                    homepage_url: None,
                    author: None,
                    logo: None,
                    description: None,
                    enabled: article.enabled,
                    file_size: size,
                    modified_at: modified,
                    source_dir: article_dir.to_string_lossy().to_string(),
                    source_type: "article".to_string(),
                    version: String::new(),
                    update_url: None,
                    tags: Vec::new(),
                    min_delay_ms: 0,
                    require_urls: Vec::new(),
                    has_explore: None,
                    capabilities: Vec::new(),
                    runtime: SourceRuntimeKind::LegacyArticle,
                });
            }
        }
        Ok(out)
    }

    async fn stream_legado_sources<F, Fut>(
        &self,
        batch_size: usize,
        all: &mut Vec<BookSourceMeta>,
        batch: &mut Vec<BookSourceMeta>,
        emit: &mut F,
    ) -> Result<(), ReaderCoreError>
    where
        F: FnMut(Vec<BookSourceMeta>, bool, Option<usize>) -> Fut,
        Fut: Future<Output = ()>,
    {
        let source_dir = self.legado_source_dir.to_string_lossy().to_string();
        let page_size = SOURCE_LIST_DB_PAGE_SIZE.max(batch_size.max(1));
        let mut cursor: Option<(i64, String)> = None;
        loop {
            let rows = self
                .source_service
                .list_rows_page_after(
                    USER_NS,
                    page_size,
                    cursor
                        .as_ref()
                        .map(|(updated_at, url)| (*updated_at, url.as_str())),
                )
                .await?;
            if rows.is_empty() {
                break;
            }
            let row_count = rows.len();
            cursor = rows
                .last()
                .map(|row| (row.updated_at, row.book_source_url.clone()));
            for row in rows {
                if let Some(meta) = BookSourceMeta::from_legado_row(&row, source_dir.clone()) {
                    Self::push_streamed_source(meta, batch_size, all, batch, emit).await;
                }
            }
            if row_count < page_size {
                break;
            }
            tokio::task::yield_now().await;
        }
        Ok(())
    }

    async fn stream_js_sources<F, Fut>(
        &self,
        batch_size: usize,
        all: &mut Vec<BookSourceMeta>,
        batch: &mut Vec<BookSourceMeta>,
        emit: &mut F,
    ) -> Result<(), ReaderCoreError>
    where
        F: FnMut(Vec<BookSourceMeta>, bool, Option<usize>) -> Fut,
        Fut: Future<Output = ()>,
    {
        for dir in self.js_source_dirs().await? {
            let mut entries = match fs::read_dir(&dir).await {
                Ok(entries) => entries,
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => continue,
                Err(err) => return Err(err.into()),
            };
            while let Some(entry) = entries.next_entry().await? {
                let path = entry.path();
                if path.extension().and_then(|value| value.to_str()) != Some("js") {
                    continue;
                }
                let file_name = entry.file_name().to_string_lossy().to_string();
                let content = fs::read_to_string(&path).await.unwrap_or_default();
                let metadata = entry.metadata().await.ok();
                if let Some(metadata) = metadata.as_ref() {
                    self.store_source_text_cache(&path, &content, metadata)
                        .await;
                }
                Self::push_streamed_source(
                    BookSourceMeta::from_js(
                        &content,
                        file_name,
                        dir.to_string_lossy().to_string(),
                        metadata.as_ref(),
                    ),
                    batch_size,
                    all,
                    batch,
                    emit,
                )
                .await;
            }
        }
        Ok(())
    }

    async fn stream_article_sources<F, Fut>(
        &self,
        batch_size: usize,
        all: &mut Vec<BookSourceMeta>,
        batch: &mut Vec<BookSourceMeta>,
        emit: &mut F,
    ) -> Result<(), ReaderCoreError>
    where
        F: FnMut(Vec<BookSourceMeta>, bool, Option<usize>) -> Fut,
        Fut: Future<Output = ()>,
    {
        let article_dir = self.reader_dir.join("sources").join("article-json");
        let mut entries = match fs::read_dir(&article_dir).await {
            Ok(entries) => entries,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(err) => return Err(err.into()),
        };
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().and_then(|value| value.to_str()) != Some("json") {
                continue;
            }
            let file_name = entry.file_name().to_string_lossy().to_string();
            let content = fs::read_to_string(&path).await.unwrap_or_default();
            let Ok(article) = serde_json::from_str::<ArticleSource>(&content) else {
                continue;
            };
            let metadata = fs::metadata(&path).await.ok();
            let modified = metadata
                .as_ref()
                .and_then(|m| {
                    m.modified()
                        .ok()
                        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                        .map(|d| d.as_secs() as i64)
                })
                .unwrap_or(0);
            let size = metadata.as_ref().map(|m| m.len()).unwrap_or(0);
            Self::push_streamed_source(
                BookSourceMeta {
                    source_key: format!("article:{}", article.source_name),
                    uuid: article.source_name.clone(),
                    file_name,
                    name: article.source_name,
                    url: article.source_url.clone(),
                    urls: vec![article.source_url.clone()],
                    homepage_url: None,
                    author: None,
                    logo: None,
                    description: None,
                    enabled: article.enabled,
                    file_size: size,
                    modified_at: modified,
                    source_dir: article_dir.to_string_lossy().to_string(),
                    source_type: "article".to_string(),
                    version: String::new(),
                    update_url: None,
                    tags: Vec::new(),
                    min_delay_ms: 0,
                    require_urls: Vec::new(),
                    has_explore: None,
                    capabilities: Vec::new(),
                    runtime: SourceRuntimeKind::LegacyArticle,
                },
                batch_size,
                all,
                batch,
                emit,
            )
            .await;
        }
        Ok(())
    }

    async fn push_streamed_source<F, Fut>(
        source: BookSourceMeta,
        batch_size: usize,
        all: &mut Vec<BookSourceMeta>,
        batch: &mut Vec<BookSourceMeta>,
        emit: &mut F,
    ) where
        F: FnMut(Vec<BookSourceMeta>, bool, Option<usize>) -> Fut,
        Fut: Future<Output = ()>,
    {
        batch.push(source);
        if batch.len() >= batch_size {
            all.extend(batch.iter().cloned());
            let items = std::mem::take(batch);
            emit(items, false, None).await;
        }
    }

    pub async fn read_source(
        &self,
        file_name: &str,
        source_dir: Option<&str>,
    ) -> Result<String, ReaderCoreError> {
        let path = self.resolve_source_file(file_name, source_dir);
        self.read_source_text_cached(&path).await
    }

    pub async fn save_js_source(
        &self,
        file_name: &str,
        content: &str,
        source_dir: Option<&str>,
    ) -> Result<(), ReaderCoreError> {
        let path = self.resolve_source_file(file_name, source_dir);
        ensure_safe_file_name(file_name)?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).await?;
        }
        fs::write(&path, content).await?;
        if let Ok(metadata) = fs::metadata(&path).await {
            self.store_source_text_cache(&path, content, &metadata)
                .await;
        } else {
            self.remove_source_text_cache(&path).await;
        }
        self.invalidate_source_list_cache().await;
        Ok(())
    }

    pub async fn delete_source(
        &self,
        file_name: &str,
        source_dir: Option<&str>,
    ) -> Result<(), ReaderCoreError> {
        if self.is_legado_file(file_name, source_dir) {
            if let Some(source) = self.get_legado_source_by_file(file_name).await? {
                self.source_service
                    .delete(USER_NS, &source.book_source_url)
                    .await?;
            }
        }
        let path = self.resolve_source_file(file_name, source_dir);
        match fs::remove_file(&path).await {
            Ok(()) => {
                self.remove_source_text_cache(&path).await;
                self.remove_legado_source_cache(file_name).await;
                self.invalidate_source_list_cache().await;
                Ok(())
            }
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                self.remove_source_text_cache(&path).await;
                self.remove_legado_source_cache(file_name).await;
                self.invalidate_source_list_cache().await;
                Ok(())
            }
            Err(err) => Err(err.into()),
        }
    }

    pub async fn toggle_source(
        &self,
        file_name: &str,
        enabled: bool,
        source_dir: Option<&str>,
    ) -> Result<(), ReaderCoreError> {
        if self.is_legado_file(file_name, source_dir) {
            let mut source = self.require_legado_source(file_name).await?;
            source.enabled = Some(enabled);
            self.persist_legado_source(file_name, &source).await?;
            return Ok(());
        }

        let path = self.resolve_source_file(file_name, source_dir);
        let content = self.read_source_text_cached(&path).await?;
        let content = set_js_meta_enabled(&content, enabled);
        fs::write(&path, &content).await?;
        if let Ok(metadata) = fs::metadata(&path).await {
            self.store_source_text_cache(&path, &content, &metadata)
                .await;
        } else {
            self.remove_source_text_cache(&path).await;
        }
        self.invalidate_source_list_cache().await;
        Ok(())
    }

    pub async fn import_legacy_json_text(
        &self,
        content: &str,
        smart_explore_sub_categories: bool,
    ) -> Result<LegacyJsonImportResult, ReaderCoreError> {
        self.import_legacy_json_text_with_progress(
            content,
            smart_explore_sub_categories,
            |_| async {},
        )
        .await
    }

    pub async fn import_legacy_json_text_with_progress<F, Fut>(
        &self,
        content: &str,
        _smart_explore_sub_categories: bool,
        mut on_progress: F,
    ) -> Result<LegacyJsonImportResult, ReaderCoreError>
    where
        F: FnMut(LegacyJsonImportProgress) -> Fut,
        Fut: Future<Output = ()>,
    {
        let value: Value = serde_json::from_str(content)?;
        let values = match value {
            Value::Array(items) => items,
            other => vec![other],
        };
        let total = values.len();

        let mut result = LegacyJsonImportResult {
            imported: 0,
            skipped: 0,
            files: Vec::new(),
            errors: Vec::new(),
        };
        let mut seen = HashSet::new();
        let mut pending_legado_sources = Vec::new();
        self.invalidate_source_list_cache().await;

        if total == 0 {
            on_progress(LegacyJsonImportProgress {
                processed: 0,
                total,
                imported: 0,
                skipped: 0,
                errors: 0,
                file_name: None,
                done: true,
            })
            .await;
            return Ok(result);
        }

        for (index, value) in values.into_iter().enumerate() {
            let mut progress_file_name = None;
            let mut handled = false;

            // Try as article source first (sourceName + ruleArticles)
            if let (Some(name), Some(_rules)) = (
                value.get("sourceName").and_then(|v| v.as_str()),
                value.get("ruleArticles"),
            ) {
                if !name.trim().is_empty() {
                    let article: ArticleSource = serde_json::from_value(value.clone())?;
                    let file_name = format!(
                        "{}.article.json",
                        article
                            .source_name
                            .replace(['/', '\\', ':', '?', '*', '"', '<', '>', '|'], "_")
                    );
                    let article_dir = self.reader_dir.join("sources").join("article-json");
                    fs::create_dir_all(&article_dir).await?;
                    let article_path = article_dir.join(&file_name);
                    fs::write(&article_path, serde_json::to_string_pretty(&article)?).await?;
                    progress_file_name = Some(file_name.clone());
                    result.imported += 1;
                    result.files.push(file_name);
                    handled = true;
                }
            }

            if !handled {
                match book_source_from_value(value) {
                    Ok(source) => {
                        if source.book_source_name.trim().is_empty()
                            || source.book_source_url.trim().is_empty()
                        {
                            result.skipped += 1;
                            result
                                .errors
                                .push("缺少 bookSourceName 或 bookSourceUrl".to_string());
                        } else if !seen.insert(source.book_source_url.clone()) {
                            result.skipped += 1;
                        } else {
                            let file_name = legado_file_name(&source);
                            self.write_legado_source_file(&file_name, &source).await?;
                            progress_file_name = Some(file_name.clone());
                            result.imported += 1;
                            result.files.push(file_name);
                            pending_legado_sources.push(source);
                        }
                    }
                    Err(err) => {
                        result.skipped += 1;
                        result.errors.push(err.to_string());
                    }
                }
            }

            let processed = index + 1;
            if processed % LEGACY_IMPORT_PROGRESS_INTERVAL == 0 || processed == total {
                if !pending_legado_sources.is_empty() {
                    let pending = std::mem::take(&mut pending_legado_sources);
                    self.source_service.save_many(USER_NS, pending).await?;
                }
                on_progress(LegacyJsonImportProgress {
                    processed,
                    total,
                    imported: result.imported,
                    skipped: result.skipped,
                    errors: result.errors.len(),
                    file_name: progress_file_name,
                    done: processed == total,
                })
                .await;
                tokio::task::yield_now().await;
            }
        }

        Ok(result)
    }

    pub async fn import_legacy_json_url(
        &self,
        url: &str,
        smart_explore_sub_categories: bool,
    ) -> Result<LegacyJsonImportResult, ReaderCoreError> {
        self.import_legacy_json_url_with_progress(url, smart_explore_sub_categories, |_| async {})
            .await
    }

    pub async fn import_legacy_json_url_with_progress<F, Fut>(
        &self,
        url: &str,
        smart_explore_sub_categories: bool,
        mut on_progress: F,
    ) -> Result<LegacyJsonImportResult, ReaderCoreError>
    where
        F: FnMut(LegacyJsonImportProgress) -> Fut,
        Fut: Future<Output = ()>,
    {
        validate_network_url(url)?;
        on_progress(LegacyJsonImportProgress {
            processed: 0,
            total: 0,
            imported: 0,
            skipped: 0,
            errors: 0,
            file_name: None,
            done: false,
        })
        .await;
        let text = self
            .book_service
            .http_client()
            .get(url)
            .send()
            .await?
            .error_for_status()?
            .text()
            .await?;
        self.import_legacy_json_text_with_progress(&text, smart_explore_sub_categories, on_progress)
            .await
    }

    // ── 书源仓库 / 在线更新（CAP-REPO）──────────────────────────

    /// Check whether a JS source has a newer version at its `@updateUrl`.
    pub async fn check_source_update(
        &self,
        file_name: &str,
        source_dir: Option<&str>,
    ) -> Result<SourceUpdateCheck, ReaderCoreError> {
        if self.is_legado_file(file_name, source_dir) {
            return Err(ReaderCoreError::Message(
                "Legado JSON 书源不支持在线更新（仅 JS 书源支持 @updateUrl）".to_string(),
            ));
        }
        let content = self.read_source(file_name, source_dir).await?;
        let update_url = first_js_meta(&content, "@updateUrl").ok_or_else(|| {
            ReaderCoreError::Message("书源未设置 @updateUrl，无法检测更新".to_string())
        })?;
        let local_version = first_js_meta(&content, "@version").unwrap_or_else(|| "1.0.0".into());
        let uuid = source_identity(&content, file_name, source_dir);

        let remote = self.download_source_text(&update_url).await?;
        if !looks_like_js_source(&remote) {
            return Err(ReaderCoreError::Message(
                "@updateUrl 返回的内容不是有效的 JS 书源".to_string(),
            ));
        }
        let remote_version = first_js_meta(&remote, "@version").unwrap_or_else(|| "1.0.0".into());

        Ok(SourceUpdateCheck {
            file_name: file_name.to_string(),
            uuid,
            has_update: version_has_update(&local_version, &remote_version),
            local_version,
            remote_version,
        })
    }

    /// Download the source at `@updateUrl` and overwrite the local file,
    /// preserving the local `@enabled` state. Validates the download before
    /// writing, so a bad fetch never corrupts the installed source.
    pub async fn apply_source_update(
        &self,
        file_name: &str,
        source_dir: Option<&str>,
    ) -> Result<(), ReaderCoreError> {
        if self.is_legado_file(file_name, source_dir) {
            return Err(ReaderCoreError::Message(
                "Legado JSON 书源不支持在线更新（仅 JS 书源支持 @updateUrl）".to_string(),
            ));
        }
        let local = self.read_source(file_name, source_dir).await?;
        let update_url = first_js_meta(&local, "@updateUrl").ok_or_else(|| {
            ReaderCoreError::Message("书源未设置 @updateUrl，无法更新".to_string())
        })?;
        let remote = self.download_source_text(&update_url).await?;
        if !looks_like_js_source(&remote) {
            return Err(ReaderCoreError::Message(
                "@updateUrl 返回的内容不是有效的 JS 书源，已取消更新".to_string(),
            ));
        }
        // Carry over the local enabled state so updating doesn't silently
        // re-enable a source the user had disabled.
        let enabled = first_js_meta(&local, "@enabled")
            .map(|v| v != "false" && v != "0")
            .unwrap_or(true);
        let merged = set_js_meta_enabled(&remote, enabled);
        self.save_js_source(file_name, &merged, source_dir).await
    }

    /// Fetch and parse a remote repository manifest (JSON).
    pub async fn repository_fetch(&self, url: &str) -> Result<RepoManifest, ReaderCoreError> {
        validate_network_url(url)?;
        let text = self.download_source_text(url).await?;
        serde_json::from_str(&text)
            .map_err(|err| ReaderCoreError::Message(format!("仓库清单 JSON 解析失败: {err}")))
    }

    /// Download a remote source and parse its metadata for an install preview.
    pub async fn repository_preview_source(
        &self,
        download_url: &str,
        expected_uuid: Option<&str>,
    ) -> Result<RemoteSourcePreview, ReaderCoreError> {
        let content = self
            .download_validated_source(download_url, expected_uuid)
            .await?;
        let file_name = file_name_from_url(download_url);
        let meta = BookSourceMeta::from_js(&content, file_name, download_url.to_string(), None);
        let has_explicit_uuid = first_js_meta(&content, "@uuid").is_some();
        Ok(RemoteSourcePreview {
            download_url: download_url.to_string(),
            meta,
            has_explicit_uuid,
        })
    }

    /// Download a remote source and install it under `file_name`.
    pub async fn repository_install(
        &self,
        download_url: &str,
        file_name: &str,
        expected_uuid: Option<&str>,
    ) -> Result<(), ReaderCoreError> {
        ensure_safe_file_name(file_name)?;
        if !file_name.to_ascii_lowercase().ends_with(".js") {
            return Err(ReaderCoreError::Message(
                "书源文件名必须以 .js 结尾".to_string(),
            ));
        }
        let content = self
            .download_validated_source(download_url, expected_uuid)
            .await?;
        self.save_js_source(file_name, &content, None).await
    }

    /// Compare a locally installed source with the repository copy, ignoring
    /// `@enabled` / `@uuid` lines (which legitimately differ per install).
    pub async fn repository_check_source_sync(
        &self,
        file_name: &str,
        download_url: &str,
        expected_uuid: Option<&str>,
    ) -> Result<RepoSourceSync, ReaderCoreError> {
        let local = self.read_source(file_name, None).await?;
        let remote = self
            .download_validated_source(download_url, expected_uuid)
            .await?;
        let local_version = first_js_meta(&local, "@version").unwrap_or_else(|| "1.0.0".into());
        let remote_version = first_js_meta(&remote, "@version").unwrap_or_else(|| "1.0.0".into());
        let uuid = expected_uuid
            .map(str::to_string)
            .or_else(|| first_js_meta(&remote, "@uuid"))
            .or_else(|| first_js_meta(&local, "@uuid"))
            .unwrap_or_default();
        let is_consistent =
            normalize_source_for_compare(&local) == normalize_source_for_compare(&remote);
        Ok(RepoSourceSync {
            file_name: file_name.to_string(),
            uuid,
            is_consistent,
            local_version,
            remote_version,
        })
    }

    /// Download text from a URL using the shared HTTP client (with SSRF guard).
    async fn download_source_text(&self, url: &str) -> Result<String, ReaderCoreError> {
        validate_network_url(url)?;
        let text = self
            .book_service
            .http_client()
            .get(url)
            .send()
            .await?
            .error_for_status()?
            .text()
            .await?;
        Ok(text)
    }

    /// Download a source, validate it looks like a JS source, and (when an
    /// expected UUID is supplied and the source declares one) verify they match.
    async fn download_validated_source(
        &self,
        url: &str,
        expected_uuid: Option<&str>,
    ) -> Result<String, ReaderCoreError> {
        let content = self.download_source_text(url).await?;
        if !looks_like_js_source(&content) {
            return Err(ReaderCoreError::Message(
                "下载的内容不是有效的 JS 书源".to_string(),
            ));
        }
        if let Some(expected) = expected_uuid.map(str::trim).filter(|v| !v.is_empty()) {
            if let Some(declared) = first_js_meta(&content, "@uuid") {
                if declared.trim() != expected {
                    return Err(ReaderCoreError::Message(format!(
                        "书源 UUID 不匹配：期望 {expected}，实际 {declared}"
                    )));
                }
            }
        }
        Ok(content)
    }

    pub async fn eval_source_capabilities(
        &self,
        file_name: &str,
        source_dir: Option<&str>,
    ) -> Result<String, ReaderCoreError> {
        if self.is_legado_file(file_name, source_dir) {
            let source = self.require_legado_source(file_name).await?;
            return Ok(legado_capabilities(&source).join(","));
        }
        let content = self.read_source(file_name, source_dir).await?;
        Ok(js_capabilities(&content).join(","))
    }

    pub async fn eval_source_entry(
        &self,
        file_name: &str,
        entry_code: &str,
        source_dir: Option<&str>,
    ) -> Result<String, ReaderCoreError> {
        if self.is_legado_file(file_name, source_dir) {
            return Err(ReaderCoreError::Message(
                "Legado JSON 书源不支持 eval；请改用 JS 书源".to_string(),
            ));
        }
        let content = self.read_source(file_name, source_dir).await?;
        let wrapper = format!(
            r#"
{content}
;(async () => {{
  {entry_code}
}})()
"#
        );
        crate::parser::js::eval_js(&wrapper, "", "")
            .map_err(|err| ReaderCoreError::Message(err.to_string()))
    }

    pub async fn save_draft(&self, file_name: &str, content: &str) -> Result<(), ReaderCoreError> {
        ensure_safe_file_name(file_name)?;
        let drafts_dir = self.reader_dir.join("drafts");
        fs::create_dir_all(&drafts_dir).await?;
        fs::write(drafts_dir.join(file_name), content).await?;
        Ok(())
    }

    pub async fn run_source_tests(
        &self,
        file_name: &str,
        source_dir: Option<&str>,
        step_filter: Option<&str>,
        timeout_secs: Option<i32>,
    ) -> Result<Value, ReaderCoreError> {
        let step_timeout = timeout_secs
            .filter(|&t| t > 0)
            .map(|t| std::time::Duration::from_secs(t as u64))
            .unwrap_or(std::time::Duration::from_secs(60));
        let overall_deadline = tokio::time::Instant::now() + step_timeout;

        #[derive(Serialize)]
        struct TestStep {
            name: String,
            status: String,
            elapsed_ms: u64,
            error: Option<String>,
            sample_count: Option<usize>,
            output_preview: Option<String>,
        }

        let enabled: Vec<String> = step_filter
            .map(|f| f.split(',').map(|s| s.trim().to_string()).collect())
            .unwrap_or_else(|| {
                vec![
                    "search".into(),
                    "bookInfo".into(),
                    "toc".into(),
                    "content".into(),
                    "explore".into(),
                ]
            });

        if self.is_legado_file(file_name, source_dir) {
            let source = self.require_legado_source(file_name).await?;
            let mut steps = Vec::new();
            // Chain context: carry URLs between steps
            let mut book_url: Option<String> = None;
            let mut toc_url: Option<String> = None;
            let mut chapter_url: Option<String> = None;

            // Helper: run a step future with timeout
            let time_limit = step_timeout;
            async fn timed_step<F, T>(
                fut: F,
                limit: std::time::Duration,
                label: &str,
            ) -> Result<T, ReaderCoreError>
            where
                F: std::future::Future<Output = Result<T, ReaderCoreError>> + Send,
                T: Send,
            {
                match tokio::time::timeout(limit, fut).await {
                    Ok(result) => result,
                    Err(_elapsed) => Err(ReaderCoreError::Message(format!(
                        "{label} 超时 ({limit:?})"
                    ))),
                }
            }

            for step_name in &enabled {
                let start = std::time::Instant::now();

                if tokio::time::Instant::now() > overall_deadline {
                    steps.push(TestStep {
                        name: step_name.clone(),
                        status: "timeout".into(),
                        elapsed_ms: start.elapsed().as_millis() as u64,
                        error: Some("整体超时".into()),
                        sample_count: None,
                        output_preview: None,
                    });
                    break;
                }

                match step_name.as_str() {
                    "search" => {
                        if source.rule_search.is_none() {
                            steps.push(TestStep {
                                name: "search".into(),
                                status: "skipped".into(),
                                elapsed_ms: 0,
                                error: Some("ruleSearch 未配置".into()),
                                sample_count: None,
                                output_preview: None,
                            });
                            continue;
                        }
                        let fut = self.search(file_name, "测试", 1, source_dir);
                        match timed_step(fut, time_limit, "search").await {
                            Ok(items) => {
                                if let Some(first) = items.first() {
                                    book_url = Some(first.book_url.clone());
                                }
                                let preview =
                                    items.first().map(|i| format!("{} - {}", i.name, i.author));
                                steps.push(TestStep {
                                    name: "search".into(),
                                    status: "passed".into(),
                                    elapsed_ms: start.elapsed().as_millis() as u64,
                                    error: None,
                                    sample_count: Some(items.len()),
                                    output_preview: preview,
                                });
                            }
                            Err(e) => {
                                steps.push(TestStep {
                                    name: "search".into(),
                                    status: "failed".into(),
                                    elapsed_ms: start.elapsed().as_millis() as u64,
                                    error: Some(e.to_string()),
                                    sample_count: None,
                                    output_preview: None,
                                });
                            }
                        }
                    }
                    "bookInfo" => {
                        let target_url =
                            book_url
                                .as_deref()
                                .unwrap_or(if source.book_source_url.is_empty() {
                                    "https://example.com"
                                } else {
                                    &source.book_source_url
                                });
                        if source.rule_book_info.is_none() {
                            steps.push(TestStep {
                                name: "bookInfo".into(),
                                status: "skipped".into(),
                                elapsed_ms: 0,
                                error: Some("ruleBookInfo 未配置".into()),
                                sample_count: None,
                                output_preview: None,
                            });
                            continue;
                        }
                        let fut = self.book_info(file_name, target_url, source_dir);
                        match timed_step(fut, time_limit, "bookInfo").await {
                            Ok(detail) => {
                                if let Some(ref tu) = detail.toc_url {
                                    if !tu.is_empty() {
                                        toc_url = Some(tu.clone());
                                    }
                                }
                                if toc_url.is_none() {
                                    toc_url = book_url.clone();
                                }
                                steps.push(TestStep {
                                    name: "bookInfo".into(),
                                    status: "passed".into(),
                                    elapsed_ms: start.elapsed().as_millis() as u64,
                                    error: None,
                                    sample_count: None,
                                    output_preview: Some(format!(
                                        "{} - {}",
                                        detail.name, detail.author
                                    )),
                                });
                            }
                            Err(e) => {
                                steps.push(TestStep {
                                    name: "bookInfo".into(),
                                    status: "failed".into(),
                                    elapsed_ms: start.elapsed().as_millis() as u64,
                                    error: Some(e.to_string()),
                                    sample_count: None,
                                    output_preview: None,
                                });
                            }
                        }
                    }
                    "toc" => {
                        let target_url = toc_url.as_deref().or(book_url.as_deref()).unwrap_or(
                            if source.book_source_url.is_empty() {
                                "https://example.com"
                            } else {
                                &source.book_source_url
                            },
                        );
                        if source.rule_toc.is_none() {
                            steps.push(TestStep {
                                name: "toc".into(),
                                status: "skipped".into(),
                                elapsed_ms: 0,
                                error: Some("ruleToc 未配置".into()),
                                sample_count: None,
                                output_preview: None,
                            });
                            continue;
                        }
                        let fut = self.chapter_list(file_name, target_url, source_dir);
                        match timed_step(fut, time_limit, "chapterList").await {
                            Ok(chapters) => {
                                if let Some(first) = chapters.first() {
                                    chapter_url = Some(first.url.clone());
                                }
                                let count = chapters.len();
                                let preview = chapters.first().map(|c| c.name.clone());
                                steps.push(TestStep {
                                    name: "toc".into(),
                                    status: "passed".into(),
                                    elapsed_ms: start.elapsed().as_millis() as u64,
                                    error: None,
                                    sample_count: Some(count),
                                    output_preview: preview,
                                });
                            }
                            Err(e) => {
                                steps.push(TestStep {
                                    name: "toc".into(),
                                    status: "failed".into(),
                                    elapsed_ms: start.elapsed().as_millis() as u64,
                                    error: Some(e.to_string()),
                                    sample_count: None,
                                    output_preview: None,
                                });
                            }
                        }
                    }
                    "content" => {
                        let target_url = chapter_url.as_deref().unwrap_or("https://example.com");
                        if source.rule_content.is_none() {
                            steps.push(TestStep {
                                name: "content".into(),
                                status: "skipped".into(),
                                elapsed_ms: 0,
                                error: Some("ruleContent 未配置".into()),
                                sample_count: None,
                                output_preview: None,
                            });
                            continue;
                        }
                        let fut = self.chapter_content(file_name, target_url, source_dir);
                        match timed_step(fut, time_limit, "chapterContent").await {
                            Ok(text) => {
                                let trimmed: String = text.chars().take(100).collect();
                                steps.push(TestStep {
                                    name: "content".into(),
                                    status: "passed".into(),
                                    elapsed_ms: start.elapsed().as_millis() as u64,
                                    error: None,
                                    sample_count: Some(text.len()),
                                    output_preview: Some(trimmed),
                                });
                            }
                            Err(e) => {
                                steps.push(TestStep {
                                    name: "content".into(),
                                    status: "failed".into(),
                                    elapsed_ms: start.elapsed().as_millis() as u64,
                                    error: Some(e.to_string()),
                                    sample_count: None,
                                    output_preview: None,
                                });
                            }
                        }
                    }
                    "explore" => {
                        if source.rule_explore.is_none() {
                            steps.push(TestStep {
                                name: "explore".into(),
                                status: "skipped".into(),
                                elapsed_ms: 0,
                                error: Some("ruleExplore 未配置".into()),
                                sample_count: None,
                                output_preview: None,
                            });
                            continue;
                        }
                        match self.explore(file_name, 1, "", source_dir).await {
                            Ok(result) => {
                                steps.push(TestStep {
                                    name: "explore".into(),
                                    status: "passed".into(),
                                    elapsed_ms: start.elapsed().as_millis() as u64,
                                    error: None,
                                    sample_count: None,
                                    output_preview: Some(
                                        serde_json::to_string(&result)
                                            .unwrap_or_default()
                                            .chars()
                                            .take(200)
                                            .collect(),
                                    ),
                                });
                            }
                            Err(e) => {
                                steps.push(TestStep {
                                    name: "explore".into(),
                                    status: "failed".into(),
                                    elapsed_ms: start.elapsed().as_millis() as u64,
                                    error: Some(e.to_string()),
                                    sample_count: None,
                                    output_preview: None,
                                });
                            }
                        }
                    }
                    _ => {}
                }
            }
            return Ok(serde_json::to_value(steps)?);
        }

        // JS 书源测试：使用现有逻辑
        let content = self.read_source(file_name, source_dir).await?;
        let runtime = JsSourceRuntime::new(file_name, content);
        let mut steps = Vec::new();

        for step_name in &enabled {
            let start = std::time::Instant::now();
            match step_name.as_str() {
                "search" => match runtime.search("test", 1) {
                    Ok(r) => {
                        let items = serde_json::to_value(r).unwrap_or_default();
                        let count = items.as_array().map(|a| a.len()).unwrap_or(0);
                        steps.push(TestStep {
                            name: "search".into(),
                            status: "passed".into(),
                            elapsed_ms: start.elapsed().as_millis() as u64,
                            error: None,
                            sample_count: Some(count),
                            output_preview: Some(format!("{} results", count)),
                        });
                    }
                    Err(e) => steps.push(TestStep {
                        name: "search".into(),
                        status: "failed".into(),
                        elapsed_ms: start.elapsed().as_millis() as u64,
                        error: Some(e.to_string()),
                        sample_count: None,
                        output_preview: None,
                    }),
                },
                "bookInfo" => match runtime.book_info("https://example.com") {
                    Ok(r) => {
                        let v = serde_json::to_value(r).unwrap_or_default();
                        steps.push(TestStep {
                            name: "bookInfo".into(),
                            status: "passed".into(),
                            elapsed_ms: start.elapsed().as_millis() as u64,
                            error: None,
                            sample_count: None,
                            output_preview: Some(
                                serde_json::to_string(&v)
                                    .unwrap_or_default()
                                    .chars()
                                    .take(200)
                                    .collect(),
                            ),
                        });
                    }
                    Err(e) => steps.push(TestStep {
                        name: "bookInfo".into(),
                        status: "failed".into(),
                        elapsed_ms: start.elapsed().as_millis() as u64,
                        error: Some(e.to_string()),
                        sample_count: None,
                        output_preview: None,
                    }),
                },
                "toc" => match runtime.chapter_list("https://example.com") {
                    Ok(r) => {
                        let v = serde_json::to_value(r).unwrap_or_default();
                        let count = v.as_array().map(|a| a.len()).unwrap_or(0);
                        steps.push(TestStep {
                            name: "toc".into(),
                            status: "passed".into(),
                            elapsed_ms: start.elapsed().as_millis() as u64,
                            error: None,
                            sample_count: Some(count),
                            output_preview: Some(format!("{} chapters", count)),
                        });
                    }
                    Err(e) => steps.push(TestStep {
                        name: "toc".into(),
                        status: "failed".into(),
                        elapsed_ms: start.elapsed().as_millis() as u64,
                        error: Some(e.to_string()),
                        sample_count: None,
                        output_preview: None,
                    }),
                },
                "content" => match runtime.chapter_content("https://example.com") {
                    Ok(r) => {
                        steps.push(TestStep {
                            name: "content".into(),
                            status: "passed".into(),
                            elapsed_ms: start.elapsed().as_millis() as u64,
                            error: None,
                            sample_count: Some(r.len()),
                            output_preview: Some(r.chars().take(100).collect()),
                        });
                    }
                    Err(e) => steps.push(TestStep {
                        name: "content".into(),
                        status: "failed".into(),
                        elapsed_ms: start.elapsed().as_millis() as u64,
                        error: Some(e.to_string()),
                        sample_count: None,
                        output_preview: None,
                    }),
                },
                "explore" => match runtime.explore(1, "") {
                    Ok(r) => steps.push(TestStep {
                        name: "explore".into(),
                        status: "passed".into(),
                        elapsed_ms: start.elapsed().as_millis() as u64,
                        error: None,
                        sample_count: None,
                        output_preview: Some(
                            serde_json::to_string(&r)
                                .unwrap_or_default()
                                .chars()
                                .take(200)
                                .collect(),
                        ),
                    }),
                    Err(e) => steps.push(TestStep {
                        name: "explore".into(),
                        status: "failed".into(),
                        elapsed_ms: start.elapsed().as_millis() as u64,
                        error: Some(e.to_string()),
                        sample_count: None,
                        output_preview: None,
                    }),
                },
                _ => {}
            }
        }
        Ok(serde_json::to_value(steps)?)
    }

    pub async fn search(
        &self,
        file_name: &str,
        keyword: &str,
        page: i32,
        source_dir: Option<&str>,
    ) -> Result<Vec<BookItem>, ReaderCoreError> {
        self.search_with_cancel(file_name, keyword, page, source_dir, None)
            .await
    }

    pub async fn search_with_cancel(
        &self,
        file_name: &str,
        keyword: &str,
        page: i32,
        source_dir: Option<&str>,
        cancel_token: Option<Arc<AtomicBool>>,
    ) -> Result<Vec<BookItem>, ReaderCoreError> {
        if cancel_token
            .as_ref()
            .map(|token| token.load(Ordering::SeqCst))
            .unwrap_or(false)
        {
            return Err(ReaderCoreError::Message("任务已取消".to_string()));
        }
        if !self.is_legado_file(file_name, source_dir) {
            let runtime = self.require_js_runtime(file_name, source_dir).await?;
            let keyword = keyword.to_string();
            return tokio::task::spawn_blocking(move || {
                runtime.search_with_cancel(&keyword, page, cancel_token)
            })
            .await
            .map_err(js_join_error)?;
        }
        let source = self.require_legado_source(file_name).await?;
        let list = self
            .book_service
            .search_book(USER_NS, &source, keyword, page)
            .await?;
        Ok(list.into_iter().map(BookItem::from).collect())
    }

    pub async fn book_info(
        &self,
        file_name: &str,
        book_url: &str,
        source_dir: Option<&str>,
    ) -> Result<BookDetail, ReaderCoreError> {
        if !self.is_legado_file(file_name, source_dir) {
            let runtime = self.require_js_runtime(file_name, source_dir).await?;
            let book_url = book_url.to_string();
            return tokio::task::spawn_blocking(move || runtime.book_info(&book_url))
                .await
                .map_err(js_join_error)?;
        }
        let source = self.require_legado_source(file_name).await?;
        let mut book = self
            .book_service
            .get_book_info(USER_NS, &source, book_url)
            .await?;
        if book.book_url.trim().is_empty() {
            book.book_url = book_url.to_string();
        }
        if book.origin.trim().is_empty() {
            book.origin = file_name.to_string();
        }
        if book.origin_name.is_none() {
            book.origin_name = Some(source.book_source_name);
        }
        Ok(BookDetail::from(book))
    }

    pub async fn chapter_list(
        &self,
        file_name: &str,
        book_url: &str,
        source_dir: Option<&str>,
    ) -> Result<Vec<ChapterItem>, ReaderCoreError> {
        self.chapter_list_with_cancel(file_name, book_url, source_dir, None)
            .await
    }

    pub async fn chapter_list_with_cancel(
        &self,
        file_name: &str,
        book_url: &str,
        source_dir: Option<&str>,
        cancel_token: Option<Arc<AtomicBool>>,
    ) -> Result<Vec<ChapterItem>, ReaderCoreError> {
        if cancel_token
            .as_ref()
            .map(|token| token.load(Ordering::SeqCst))
            .unwrap_or(false)
        {
            return Err(ReaderCoreError::Message("任务已取消".to_string()));
        }
        if !self.is_legado_file(file_name, source_dir) {
            let runtime = self.require_js_runtime(file_name, source_dir).await?;
            let book_url = book_url.to_string();
            return tokio::task::spawn_blocking(move || {
                runtime.chapter_list_with_cancel(&book_url, cancel_token)
            })
            .await
            .map_err(js_join_error)?;
        }
        let source = self.require_legado_source(file_name).await?;
        let list = self
            .book_service
            .get_chapter_list(USER_NS, &source, book_url)
            .await?;
        Ok(list.into_iter().map(ChapterItem::from).collect())
    }

    pub async fn chapter_content(
        &self,
        file_name: &str,
        chapter_url: &str,
        source_dir: Option<&str>,
    ) -> Result<String, ReaderCoreError> {
        self.chapter_content_with_cancel(file_name, chapter_url, source_dir, None)
            .await
    }

    pub async fn chapter_content_with_cancel(
        &self,
        file_name: &str,
        chapter_url: &str,
        source_dir: Option<&str>,
        cancel_token: Option<Arc<AtomicBool>>,
    ) -> Result<String, ReaderCoreError> {
        if cancel_token
            .as_ref()
            .map(|token| token.load(Ordering::SeqCst))
            .unwrap_or(false)
        {
            return Err(ReaderCoreError::Message("任务已取消".to_string()));
        }
        if !self.is_legado_file(file_name, source_dir) {
            let runtime = self.require_js_runtime(file_name, source_dir).await?;
            let chapter_url = chapter_url.to_string();
            return tokio::task::spawn_blocking(move || {
                runtime.chapter_content_with_cancel(&chapter_url, cancel_token)
            })
            .await
            .map_err(js_join_error)?;
        }
        let source = self.require_legado_source(file_name).await?;
        self.book_service
            .get_content(
                USER_NS,
                &content_cache_book_key(file_name, chapter_url),
                &source,
                chapter_url,
            )
            .await
            .map_err(ReaderCoreError::from)
    }

    pub async fn purchase_chapter(
        &self,
        file_name: &str,
        chapter_url: &str,
        _chapter: Option<&Value>,
        source_dir: Option<&str>,
    ) -> Result<Value, ReaderCoreError> {
        if !self.is_legado_file(file_name, source_dir) {
            let runtime = self.require_js_runtime(file_name, source_dir).await?;
            let chapter_url = chapter_url.to_string();
            return tokio::task::spawn_blocking(move || {
                runtime.call_function("purchaseChapter", &[JsSourceArg::String(chapter_url)])
            })
            .await
            .map_err(js_join_error)?;
        }
        Ok(
            json!({ "ok": false, "purchased": false, "unsupported": true, "message": "Legado 规则书源不支持自动购买，请手动处理" }),
        )
    }

    pub async fn source_call_fn(
        &self,
        file_name: &str,
        fn_name: &str,
        args: Vec<Value>,
        source_dir: Option<&str>,
    ) -> Result<Value, ReaderCoreError> {
        if !self.is_legado_file(file_name, source_dir) {
            let runtime = self.require_js_runtime(file_name, source_dir).await?;
            let fn_name = fn_name.to_string();
            let js_args: Vec<JsSourceArg> = args.into_iter().map(value_to_js_source_arg).collect();
            return tokio::task::spawn_blocking(move || runtime.call_function(&fn_name, &js_args))
                .await
                .map_err(js_join_error)?;
        }
        if fn_name == LEGADO_BROWSER_ACTION_FN {
            return self.legado_browser_action(file_name, args).await;
        }
        Err(ReaderCoreError::Message(format!(
            "Legado 规则书源不支持自定义 JS 函数调用: {fn_name}"
        )))
    }

    async fn legado_browser_action(
        &self,
        file_name: &str,
        args: Vec<Value>,
    ) -> Result<Value, ReaderCoreError> {
        let payload = args.first().cloned().unwrap_or(Value::Null);
        let expression = payload
            .get("expression")
            .and_then(Value::as_str)
            .map(str::trim)
            .unwrap_or_default();
        if expression.is_empty() {
            return Err(ReaderCoreError::Message("缺少浏览器动作表达式".to_string()));
        }

        let chapter_url = payload
            .get("chapterUrl")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let chapter_title = payload
            .get("chapterTitle")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let chapter_index = payload
            .get("chapterIndex")
            .and_then(Value::as_i64)
            .and_then(|value| i32::try_from(value).ok());
        let source = self.require_legado_source(file_name).await?;
        let script = legado_browser_action_script(expression)?;

        tokio::task::spawn_blocking(move || {
            let toc_url = derive_toc_url_from_chapter_url(&chapter_url);
            let raw = with_js_source(
                source.js_lib.as_deref(),
                source.login_url.as_deref(),
                Some(&source.book_source_name),
                Some(&source.book_source_url),
                toc_url.as_deref(),
                Some(&chapter_url),
                Some(&chapter_title),
                chapter_index,
                || eval_js(&script, "", &chapter_url),
            )?;
            let value = serde_json::from_str::<Value>(&raw).unwrap_or_else(|_| {
                json!({
                    "ok": false,
                    "error": "浏览器动作返回非 JSON",
                    "raw": raw,
                })
            });
            Ok(value)
        })
        .await
        .map_err(js_join_error)?
    }

    pub async fn explore(
        &self,
        file_name: &str,
        page: i32,
        category: &str,
        source_dir: Option<&str>,
    ) -> Result<Value, ReaderCoreError> {
        if !self.is_legado_file(file_name, source_dir) {
            let runtime = self.require_js_runtime(file_name, source_dir).await?;
            let category = category.to_string();
            return tokio::task::spawn_blocking(move || runtime.explore(page, &category))
                .await
                .map_err(js_join_error)?;
        }
        let source = self.require_legado_source(file_name).await?;
        if category == "GETALL" {
            let kinds = self.book_service.explore_kinds(&source)?;
            return Ok(serde_json::to_value(kinds)?);
        }
        let target = category
            .split_once("::")
            .map(|(_, url)| url)
            .unwrap_or(category);
        let books = self
            .book_service
            .explore_book(USER_NS, &source, target, page)
            .await?;
        let items = books.into_iter().map(BookItem::from).collect::<Vec<_>>();
        Ok(serde_json::to_value(items)?)
    }

    pub async fn shelf_list(&self) -> Result<Vec<ShelfBook>, ReaderCoreError> {
        let books = self.read_shelf_books().await?;
        Ok(books.into_values().collect())
    }

    pub async fn shelf_add(
        &self,
        book: AddBookPayload,
        file_name: &str,
        source_name: &str,
    ) -> Result<ShelfBook, ReaderCoreError> {
        let mut books = self.read_shelf_books().await?;
        let source_dir = clean_optional_string(book.source_dir.clone());
        let exact_id = shelf_id(&book.book_url, file_name, source_dir.as_deref());
        let legacy_id = shelf_id(&book.book_url, file_name, None);
        let id = if exact_id != legacy_id && !books.contains_key(&exact_id) {
            match books.get(&legacy_id) {
                Some(item)
                    if item.source_dir.as_deref().unwrap_or_default().is_empty()
                        || item.source_dir.as_deref() == source_dir.as_deref() =>
                {
                    legacy_id
                }
                _ => exact_id,
            }
        } else {
            exact_id
        };
        let now = now_ms();
        let item = books.entry(id.clone()).or_insert_with(|| ShelfBook {
            id: id.clone(),
            name: book.name.clone(),
            author: book.author.clone().unwrap_or_default(),
            cover_url: book.cover_url.clone(),
            cover_referer: None,
            intro: book.intro.clone(),
            kind: book.kind.clone(),
            group_id: book.group_id.clone(),
            book_url: book.book_url.clone(),
            file_name: file_name.to_string(),
            source_dir: source_dir.clone(),
            source_name: source_name.to_string(),
            last_chapter: book.last_chapter.clone(),
            added_at: now,
            last_read_at: now,
            read_chapter_index: -1,
            read_chapter_url: None,
            total_chapters: 0,
            source_type: book
                .source_type
                .clone()
                .unwrap_or_else(|| "novel".to_string()),
            read_page_index: -1,
            read_scroll_ratio: -1.0,
            read_playback_time: -1.0,
            reader_settings: None,
            is_private: false,
        });
        item.name = book.name;
        item.author = book.author.unwrap_or_default();
        item.cover_url = book.cover_url;
        item.intro = book.intro;
        item.kind = book.kind;
        item.group_id = book.group_id;
        item.last_chapter = book.last_chapter;
        item.source_type = book.source_type.unwrap_or_else(|| item.source_type.clone());
        if source_dir.is_some() {
            item.source_dir = source_dir;
        }
        item.last_read_at = now;
        let result = item.clone();
        self.write_shelf_books(&books).await?;
        Ok(result)
    }

    pub async fn shelf_get(&self, id: &str) -> Result<ShelfBook, ReaderCoreError> {
        self.read_shelf_books()
            .await?
            .remove(id)
            .ok_or_else(|| ReaderCoreError::Message(format!("书架中不存在: {id}")))
    }

    pub async fn shelf_remove(&self, id: &str) -> Result<(), ReaderCoreError> {
        let mut books = self.read_shelf_books().await?;
        books.remove(id);
        self.write_shelf_books(&books).await?;
        let _ = fs::remove_dir_all(self.shelf_book_dir(id)).await;
        Ok(())
    }

    pub async fn shelf_update_progress(
        &self,
        id: &str,
        chapter_index: i32,
        chapter_url: &str,
        page_index: Option<i32>,
        scroll_ratio: Option<f64>,
        playback_time: Option<f64>,
        reader_settings: Option<String>,
    ) -> Result<(), ReaderCoreError> {
        let mut books = self.read_shelf_books().await?;
        let book = books
            .get_mut(id)
            .ok_or_else(|| ReaderCoreError::Message(format!("书架中不存在: {id}")))?;
        book.read_chapter_index = chapter_index;
        book.read_chapter_url = Some(chapter_url.to_string());
        book.last_read_at = now_ms();
        if let Some(value) = page_index {
            book.read_page_index = value;
        }
        if let Some(value) = scroll_ratio {
            book.read_scroll_ratio = value;
        }
        if let Some(value) = playback_time {
            book.read_playback_time = value;
        }
        if reader_settings.is_some() {
            book.reader_settings = reader_settings;
        }
        self.write_shelf_books(&books).await
    }

    pub async fn shelf_set_private(
        &self,
        id: &str,
        is_private: bool,
    ) -> Result<(), ReaderCoreError> {
        let mut books = self.read_shelf_books().await?;
        let book = books
            .get_mut(id)
            .ok_or_else(|| ReaderCoreError::Message(format!("书架中不存在: {id}")))?;
        book.is_private = is_private;
        self.write_shelf_books(&books).await
    }

    pub async fn shelf_update_book(
        &self,
        book: UpdateShelfBookPayload,
        chapters: Option<Vec<CachedChapter>>,
    ) -> Result<ShelfBook, ReaderCoreError> {
        let mut books = self.read_shelf_books().await?;
        let now = now_ms();
        let previous = books.get(&book.id).cloned();
        let source_dir = clean_optional_string(book.source_dir)
            .or_else(|| previous.as_ref().and_then(|item| item.source_dir.clone()));
        let item = ShelfBook {
            id: book.id.clone(),
            name: book.name,
            author: book.author.unwrap_or_default(),
            cover_url: book.cover_url,
            cover_referer: None,
            intro: book.intro,
            kind: book.kind,
            group_id: book.group_id,
            book_url: book.book_url,
            file_name: book.file_name,
            source_dir,
            source_name: book.source_name,
            last_chapter: book.last_chapter,
            added_at: book.added_at.unwrap_or(now),
            last_read_at: book.last_read_at.unwrap_or(now),
            read_chapter_index: book.read_chapter_index,
            read_chapter_url: book.read_chapter_url,
            total_chapters: book.total_chapters,
            source_type: book.source_type,
            read_page_index: book.read_page_index.unwrap_or(-1),
            read_scroll_ratio: book.read_scroll_ratio.unwrap_or(-1.0),
            read_playback_time: book.read_playback_time.unwrap_or(-1.0),
            reader_settings: book.reader_settings,
            is_private: book.is_private.unwrap_or(false),
        };
        books.insert(item.id.clone(), item.clone());
        self.write_shelf_books(&books).await?;
        if let Some(chapters) = chapters {
            self.shelf_save_chapters(&item.id, chapters).await?;
        }
        Ok(item)
    }

    pub async fn shelf_save_chapters(
        &self,
        id: &str,
        chapters: Vec<CachedChapter>,
    ) -> Result<(), ReaderCoreError> {
        self.write_json_file(&self.shelf_book_dir(id).join("chapters.json"), &chapters)
            .await
    }

    pub async fn shelf_get_chapters(
        &self,
        id: &str,
    ) -> Result<Vec<CachedChapter>, ReaderCoreError> {
        self.read_json_file(&self.shelf_book_dir(id).join("chapters.json"))
            .await
    }

    pub async fn shelf_save_content(
        &self,
        id: &str,
        chapter_index: i32,
        content: &str,
    ) -> Result<(), ReaderCoreError> {
        let path = self.content_path(id, chapter_index);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).await?;
        }
        fs::write(path, content).await?;
        Ok(())
    }

    pub async fn shelf_get_content(
        &self,
        id: &str,
        chapter_index: i32,
    ) -> Result<Option<String>, ReaderCoreError> {
        let path = self.content_path(id, chapter_index);
        match fs::read_to_string(path).await {
            Ok(value) => Ok(Some(value)),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(err) => Err(err.into()),
        }
    }

    pub async fn shelf_delete_content(
        &self,
        id: &str,
        chapter_index: i32,
    ) -> Result<(), ReaderCoreError> {
        match fs::remove_file(self.content_path(id, chapter_index)).await {
            Ok(()) => Ok(()),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(err) => Err(err.into()),
        }
    }

    pub async fn shelf_cached_indices(&self, id: &str) -> Result<Vec<i32>, ReaderCoreError> {
        let dir = self.shelf_book_dir(id).join("content");
        let mut out = Vec::new();
        let mut entries = match fs::read_dir(dir).await {
            Ok(entries) => entries,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(out),
            Err(err) => return Err(err.into()),
        };
        while let Some(entry) = entries.next_entry().await? {
            let name = entry.file_name().to_string_lossy().to_string();
            if let Some(index) = name
                .strip_suffix(".txt")
                .and_then(|value| value.parse::<i32>().ok())
            {
                out.push(index);
            }
        }
        out.sort_unstable();
        Ok(out)
    }

    pub async fn shelf_restore_source_switch(
        &self,
        id: &str,
    ) -> Result<SourceSwitchRestoreResult, ReaderCoreError> {
        Ok(SourceSwitchRestoreResult {
            book: self.shelf_get(id).await?,
            chapters: self.shelf_get_chapters(id).await?,
        })
    }

    pub async fn shelf_get_episode_progress(
        &self,
        id: &str,
    ) -> Result<EpisodeProgressMap, ReaderCoreError> {
        self.read_json_file(&self.shelf_book_dir(id).join("episode-progress.json"))
            .await
    }

    pub async fn shelf_save_episode_progress(
        &self,
        id: &str,
        chapter_url: &str,
        time: f64,
        duration: f64,
    ) -> Result<(), ReaderCoreError> {
        let mut map = self.shelf_get_episode_progress(id).await?;
        map.insert(
            chapter_url.to_string(),
            EpisodeProgress {
                time,
                duration,
                last_played_at: now_ms(),
            },
        );
        self.write_json_file(&self.shelf_book_dir(id).join("episode-progress.json"), &map)
            .await
    }

    pub async fn prefetch_chapters<F>(
        &self,
        id: &str,
        file_name: &str,
        source_dir: Option<&str>,
        start_index: Option<i32>,
        count: Option<i32>,
        cancel_token: Option<Arc<AtomicBool>>,
        on_progress: Option<F>,
    ) -> Result<i32, ReaderCoreError>
    where
        F: Fn(i32, i32, i32) + Send + Sync + 'static,
    {
        // 同一本书同时只允许一个缓存任务：新任务取消旧任务。
        let token = cancel_token.unwrap_or_else(|| Arc::new(AtomicBool::new(false)));
        {
            let mut tasks = self.prefetch_tasks.lock().await;
            if let Some(prev) = tasks.insert(id.to_string(), token.clone()) {
                prev.store(true, Ordering::SeqCst);
            }
        }
        let result = self
            .prefetch_chapters_inner(
                id,
                file_name,
                source_dir,
                start_index,
                count,
                &token,
                on_progress,
            )
            .await;
        let mut tasks = self.prefetch_tasks.lock().await;
        if let Some(current) = tasks.get(id) {
            if Arc::ptr_eq(current, &token) {
                tasks.remove(id);
            }
        }
        result
    }

    #[allow(clippy::too_many_arguments)]
    async fn prefetch_chapters_inner<F>(
        &self,
        id: &str,
        file_name: &str,
        source_dir: Option<&str>,
        start_index: Option<i32>,
        count: Option<i32>,
        cancel_token: &Arc<AtomicBool>,
        on_progress: Option<F>,
    ) -> Result<i32, ReaderCoreError>
    where
        F: Fn(i32, i32, i32) + Send + Sync + 'static,
    {
        let chapters = self.shelf_get_chapters(id).await?;
        let start = start_index.unwrap_or(0).max(0);
        // count：None 或负数表示缓存到书末，0 表示关闭，正数为向后缓存的章节数。
        let end = match count {
            Some(0) => return Ok(0),
            Some(n) if n > 0 => start.saturating_add(n),
            _ => i32::MAX,
        };
        // 总目标章节数（用于进度上报）
        let total = (end.min(chapters.len() as i32) - start).max(0) as i32;
        let mut fetched = 0;
        let max_retries = 2usize;
        // 章节间最小延迟（毫秒），避免对书源连续高频请求导致 IP 拉黑。
        let inter_chapter_delay_ms: u64 = 1500;
        for chapter in chapters
            .iter()
            .filter(|c| c.index >= start && c.index < end)
        {
            if cancel_token.load(Ordering::SeqCst) {
                return Err(ReaderCoreError::Message("任务已取消".to_string()));
            }
            if chapter.url.is_empty() {
                continue;
            }
            let chapter_idx = chapter.index;
            if self.shelf_get_content(id, chapter_idx).await?.is_some() {
                continue;
            }
            let mut content = Err(ReaderCoreError::Message("未尝试".into()));
            for attempt in 0..=max_retries {
                match self
                    .chapter_content_with_cancel(
                        file_name,
                        &chapter.url,
                        source_dir,
                        Some(cancel_token.clone()),
                    )
                    .await
                {
                    Ok(c) => {
                        content = Ok(c);
                        break;
                    }
                    Err(e) => {
                        if cancel_token.load(Ordering::SeqCst) {
                            return Err(ReaderCoreError::Message("任务已取消".to_string()));
                        }
                        tracing::warn!(
                            "prefetch retry {}/{} for chapter {}: {e}",
                            attempt + 1,
                            max_retries,
                            chapter_idx
                        );
                        // 失败退避，避免对书源连续高频重试。
                        cancellable_sleep(
                            Duration::from_millis(1000 * (attempt as u64 + 1)),
                            cancel_token,
                        )
                        .await?;
                    }
                }
            }
            let content = content?;
            self.shelf_save_content(id, chapter_idx, &content).await?;
            fetched += 1;
            // 每章成功后上报进度
            if let Some(ref cb) = on_progress {
                cb(fetched, total, chapter_idx);
            }
            // 每章间等待，避免请求过于密集
            if fetched < total && total > 1 {
                cancellable_sleep(Duration::from_millis(inter_chapter_delay_ms), cancel_token)
                    .await?;
            }
        }
        Ok(fetched)
    }

    /// 创建本地备份：导出所有书架数据 + 书源列表到 JSON 文件
    pub async fn create_backup(&self) -> Result<String, ReaderCoreError> {
        let shelf_books = self.read_shelf_books().await?;
        let sources = self.source_service.list(USER_NS).await?;
        let backup = serde_json::json!({
            "version": 1,
            "createdAt": now_ts(),
            "shelfBooks": shelf_books.values().collect::<Vec<_>>(),
            "bookSources": sources,
        });
        let backup_dir = self.reader_dir.join("backups");
        fs::create_dir_all(&backup_dir).await?;
        let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
        let file_name = format!("legado_backup_{}.json", timestamp);
        let path = backup_dir.join(&file_name);
        fs::write(&path, serde_json::to_string_pretty(&backup)?).await?;
        Ok(path.to_string_lossy().to_string())
    }

    /// 从备份文件恢复书架数据
    pub async fn restore_backup(&self, backup_path: &str) -> Result<i32, ReaderCoreError> {
        let raw = fs::read_to_string(backup_path).await?;
        match serde_json::from_str::<serde_json::Value>(&raw) {
            Ok(v) => {
                let mut restored = 0i32;
                if let Some(books) = v.get("shelfBooks").and_then(|b| b.as_array()) {
                    let mut shelf = self.read_shelf_books().await?;
                    for book_val in books {
                        if let Ok(book) = serde_json::from_value::<ShelfBook>(book_val.clone()) {
                            shelf.insert(book.id.clone(), book);
                            restored += 1;
                        }
                    }
                    self.write_shelf_books(&shelf).await?;
                }
                if let Some(srcs) = v.get("bookSources").and_then(|s| s.as_array()) {
                    for src_val in srcs {
                        if let Ok(source) = serde_json::from_value::<BookSource>(src_val.clone()) {
                            let file_name = legado_file_name(&source);
                            self.persist_legado_source(&file_name, &source).await?;
                        }
                    }
                }
                Ok(restored)
            }
            Err(e) => Err(ReaderCoreError::Message(format!("备份文件格式无效: {e}"))),
        }
    }

    /// EPUB 导出 stub（后续实现）
    pub async fn export_book_epub(&self, id: &str, save_path: &str) -> Result<(), ReaderCoreError> {
        let _ = (id, save_path);
        Err(ReaderCoreError::Message(
            "EPUB 导出尚未实现，请使用 TXT 或 JSON 格式".into(),
        ))
    }

    pub async fn config_read(&self, scope: &str, key: &str) -> Result<String, ReaderCoreError> {
        Ok(self
            .config_read_json(scope, key)
            .await?
            .and_then(|value| value.as_str().map(str::to_string))
            .unwrap_or_default())
    }

    pub async fn config_write(
        &self,
        scope: &str,
        key: &str,
        value: &str,
    ) -> Result<(), ReaderCoreError> {
        self.config_write_json(scope, key, &Value::String(value.to_string()))
            .await
    }

    pub async fn config_read_json(
        &self,
        scope: &str,
        key: &str,
    ) -> Result<Option<Value>, ReaderCoreError> {
        let namespace = config_namespace(scope);
        Ok(self.document_service.get_value(&namespace, key).await?)
    }

    pub async fn config_write_json(
        &self,
        scope: &str,
        key: &str,
        value: &Value,
    ) -> Result<(), ReaderCoreError> {
        let namespace = config_namespace(scope);
        self.document_service
            .set_value(&namespace, key, value)
            .await?;
        Ok(())
    }

    pub async fn config_delete_key(&self, scope: &str, key: &str) -> Result<(), ReaderCoreError> {
        let namespace = config_namespace(scope);
        sqlx::query("DELETE FROM json_documents WHERE namespace=?1 AND name=?2")
            .bind(namespace)
            .bind(key)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn config_read_all(&self, scope: &str) -> Result<String, ReaderCoreError> {
        let namespace = config_namespace(scope);
        let rows = sqlx::query("SELECT name, json FROM json_documents WHERE namespace=?1")
            .bind(namespace)
            .fetch_all(&self.pool)
            .await?;
        let mut object = serde_json::Map::new();
        for row in rows {
            let key: String = row.get("name");
            let raw: String = row.get("json");
            let value = serde_json::from_str(&raw).unwrap_or(Value::String(raw));
            object.insert(key, value);
        }
        Ok(Value::Object(object).to_string())
    }

    pub async fn config_clear(&self, scope: &str) -> Result<(), ReaderCoreError> {
        let namespace = config_namespace(scope);
        sqlx::query("DELETE FROM json_documents WHERE namespace=?1")
            .bind(namespace)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn frontend_storage_list(
        &self,
        namespace: &str,
    ) -> Result<Vec<FrontendStorageEntry>, ReaderCoreError> {
        let storage_ns = frontend_namespace(namespace);
        let rows =
            sqlx::query("SELECT name, json FROM json_documents WHERE namespace=?1 ORDER BY name")
                .bind(storage_ns)
                .fetch_all(&self.pool)
                .await?;
        Ok(rows
            .into_iter()
            .map(|row| {
                let key: String = row.get("name");
                let raw: String = row.get("json");
                let value = serde_json::from_str::<String>(&raw).unwrap_or(raw);
                FrontendStorageEntry { key, value }
            })
            .collect())
    }

    pub async fn frontend_storage_set(
        &self,
        namespace: &str,
        key: &str,
        value: &str,
    ) -> Result<(), ReaderCoreError> {
        let storage_ns = frontend_namespace(namespace);
        self.document_service
            .set_value(&storage_ns, key, &value.to_string())
            .await?;
        Ok(())
    }

    pub async fn frontend_storage_remove(
        &self,
        namespace: &str,
        key: &str,
    ) -> Result<(), ReaderCoreError> {
        let storage_ns = frontend_namespace(namespace);
        sqlx::query("DELETE FROM json_documents WHERE namespace=?1 AND name=?2")
            .bind(storage_ns)
            .bind(key)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn frontend_storage_list_namespaces(
        &self,
    ) -> Result<Vec<FrontendStorageNamespaceSummary>, ReaderCoreError> {
        let rows = sqlx::query(
            "SELECT namespace, COUNT(*) AS count FROM json_documents WHERE namespace LIKE ?1 GROUP BY namespace ORDER BY namespace",
        )
        .bind(format!("{FRONTEND_STORAGE_PREFIX}%"))
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|row| {
                let namespace: String = row.get("namespace");
                let count: i64 = row.get("count");
                FrontendStorageNamespaceSummary {
                    namespace: namespace
                        .strip_prefix(FRONTEND_STORAGE_PREFIX)
                        .unwrap_or(&namespace)
                        .to_string(),
                    count: count as usize,
                }
            })
            .collect())
    }

    pub async fn app_config_get_all(&self) -> Result<Value, ReaderCoreError> {
        Ok(load_app_config(&self.pool).await)
    }

    pub async fn app_config_set(&self, key: &str, value: &Value) -> Result<(), ReaderCoreError> {
        self.config_write_json(APP_CONFIG_SCOPE, key, value).await?;
        // The JS HTTP per-host rate floor applies live (no restart prompt in UI);
        // proxy / TLS / UA changes still take effect on next launch per the panel note.
        if key == "request_min_delay_ms" {
            let ms = value
                .as_u64()
                .or_else(|| value.as_str().and_then(|s| s.trim().parse::<u64>().ok()))
                .unwrap_or(300);
            crate::parser::js::set_js_http_min_delay_ms(ms);
        }
        if key == "engine_timeout_secs" {
            let secs = value
                .as_u64()
                .or_else(|| value.as_str().and_then(|s| s.trim().parse::<u64>().ok()))
                .unwrap_or(30);
            crate::parser::js::set_js_engine_timeout_secs(secs);
        }
        Ok(())
    }

    pub async fn config_list_scopes(&self) -> Result<Vec<String>, ReaderCoreError> {
        self.document_service
            .list_namespaces()
            .await
            .map_err(|e| ReaderCoreError::Message(e.to_string()))
    }

    pub async fn app_config_reset(&self, key: &str) -> Result<(), ReaderCoreError> {
        self.config_delete_key(APP_CONFIG_SCOPE, key).await
    }

    // ── WebDAV 同步（CAP-SYNC）───────────────────────────────

    pub async fn sync_set_credentials(&self, password: &str) -> Result<(), ReaderCoreError> {
        let path = self.sync_credentials_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).await?;
        }
        let payload = json!({
            "webdavPassword": password,
            "updatedAt": now_ms(),
        });
        fs::write(path, serde_json::to_string_pretty(&payload)?).await?;
        Ok(())
    }

    pub async fn sync_get_credentials(&self) -> Result<SyncCredentials, ReaderCoreError> {
        Ok(SyncCredentials {
            password: String::new(),
            password_set: self.read_sync_password().await?.is_some(),
        })
    }

    pub async fn sync_clear_credentials(&self) -> Result<(), ReaderCoreError> {
        match fs::remove_file(self.sync_credentials_path()).await {
            Ok(()) => Ok(()),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(err) => Err(err.into()),
        }
    }

    pub async fn sync_get_status(&self) -> Result<SyncStatus, ReaderCoreError> {
        let mut status = self.sync_runtime.status().await;
        let config = self.sync_config().await?;
        status.enabled = config.enabled
            && config.provider == "webdav"
            && !config.base_url.trim().is_empty()
            && !config.enabled_domains.is_empty();
        status.dirty_domains = config.enabled_domains;
        status.conflict_count = self
            .sync_runtime
            .conflicts()
            .await
            .into_iter()
            .filter(|item| !item.resolved)
            .count();
        Ok(status)
    }

    pub async fn sync_test_connection(
        &self,
        password: Option<&str>,
    ) -> Result<SyncConnectionTestResult, ReaderCoreError> {
        let client = self.sync_webdav_client(password).await?;
        match client.test_connection().await {
            Ok(()) => Ok(SyncConnectionTestResult {
                ok: true,
                message: "WebDAV 连接测试通过".to_string(),
            }),
            Err(err) => Ok(SyncConnectionTestResult {
                ok: false,
                message: sanitize_sync_error(&err.to_string()),
            }),
        }
    }

    pub async fn sync_now(
        &self,
        mode: &str,
        domains: Option<Vec<String>>,
        conflict_strategy: Option<&str>,
    ) -> Result<SyncRunSummary, ReaderCoreError> {
        let current = self.sync_runtime.status().await;
        if current.running {
            return Err(ReaderCoreError::Message("同步任务正在运行".to_string()));
        }
        self.sync_runtime.set_running(true).await;
        let result = self.sync_now_inner(mode, domains, conflict_strategy).await;
        match &result {
            Ok(summary) => {
                self.sync_runtime
                    .mark_success(summary.message.clone(), summary.conflict_count)
                    .await;
            }
            Err(err) => {
                self.sync_runtime
                    .mark_failure(sanitize_sync_error(&err.to_string()))
                    .await;
            }
        }
        result
    }

    pub async fn sync_list_conflicts(&self) -> Result<Vec<SyncConflict>, ReaderCoreError> {
        Ok(self.sync_runtime.conflicts().await)
    }

    pub async fn sync_resolve_conflict(
        &self,
        conflict_id: &str,
        action: &str,
    ) -> Result<Vec<SyncClientState>, ReaderCoreError> {
        let conflict = self
            .sync_runtime
            .resolve_conflict(conflict_id)
            .await
            .ok_or_else(|| ReaderCoreError::Message("同步冲突不存在或已处理".to_string()))?;
        match action {
            "local" => {
                let client = self.sync_webdav_client(None).await?;
                client.put_domain(&conflict.domain, &conflict.local).await?;
                Ok(Vec::new())
            }
            "remote" => {
                let client_state = self
                    .apply_sync_domain(&conflict.domain, conflict.remote.clone())
                    .await?;
                Ok(client_state.into_iter().collect())
            }
            "ignore" => Ok(Vec::new()),
            other => Err(ReaderCoreError::Message(format!(
                "不支持的冲突处理动作: {other}"
            ))),
        }
    }

    pub async fn sync_client_state_set(
        &self,
        domain: &str,
        value: Value,
    ) -> Result<(), ReaderCoreError> {
        match domain {
            "reader_settings" | "source_flags" => {
                self.sync_runtime.set_client_state(domain, value).await;
                Ok(())
            }
            other => Err(ReaderCoreError::Message(format!(
                "不支持的前端同步状态域: {other}"
            ))),
        }
    }

    pub async fn sync_report_reader_session(
        &self,
        session: ReaderSessionPayload,
    ) -> Result<(), ReaderCoreError> {
        self.sync_runtime.set_reader_session(session).await;
        Ok(())
    }

    pub async fn sync_v2_sync_reading_progress(
        &self,
        book_id: &str,
    ) -> Result<SyncV2ProgressResult, ReaderCoreError> {
        let local = self.reading_progress_snapshot().await?;
        let summary = self
            .sync_now("sync", Some(vec!["reading_progress".to_string()]), None)
            .await?;
        let remote = self
            .sync_runtime
            .conflicts()
            .await
            .into_iter()
            .find(|item| item.domain == "reading_progress" && !item.resolved)
            .and_then(|item| item.remote.get("books").cloned())
            .and_then(|books| find_progress_for_book(&books, book_id));
        Ok(SyncV2ProgressResult {
            status: summary.status,
            message: summary.message,
            local: find_progress_for_book(&local["books"], book_id),
            remote,
        })
    }

    pub async fn sync_notify_lifecycle(&self, event: &str) -> Result<(), ReaderCoreError> {
        if !matches!(event, "startup" | "resume" | "background") {
            return Err(ReaderCoreError::Message(format!(
                "未知同步生命周期事件: {event}"
            )));
        }
        Ok(())
    }

    pub async fn sync_client_states_for_domains(&self, domains: &[String]) -> Vec<SyncClientState> {
        self.sync_runtime.client_states_for_domains(domains).await
    }

    pub async fn resolve_audio_cache(
        &self,
        url: &str,
        referer: &str,
    ) -> Result<String, ReaderCoreError> {
        let cache_dir = self.reader_dir.join("cache").join("audio");
        fs::create_dir_all(&cache_dir).await?;
        let ext = url
            .split('?')
            .next()
            .and_then(|path| path.rsplit('.').next())
            .filter(|ext| !ext.is_empty() && ext.len() <= 5)
            .unwrap_or("mp3");
        let file_name = format!("{}.{}", md5_hex(url), ext);
        let file_path = cache_dir.join(&file_name);
        if file_path.exists() {
            return Ok(file_path.to_string_lossy().to_string());
        }
        let client = reqwest::Client::builder()
            .gzip(true)
            .brotli(true)
            .timeout(std::time::Duration::from_secs(60))
            .build()?;
        let response = client
            .get(url)
            .header("Referer", referer)
            .header(
                "User-Agent",
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36",
            )
            .send()
            .await?;
        let bytes = response.bytes().await?;
        fs::write(&file_path, &bytes).await?;
        Ok(file_path.to_string_lossy().to_string())
    }

    pub fn eval_repl(
        &self,
        code: &str,
        context_file: Option<&str>,
    ) -> Result<String, ReaderCoreError> {
        let wrapper = if let Some(ctx) = context_file {
            format!(
                r#"
{ctx}
;(async () => {{
  {code}
}})()
"#
            )
        } else {
            format!(
                r#"
;(async () => {{
  {code}
}})()
"#
            )
        };
        let result = crate::parser::js::eval_js(&wrapper, "", "")
            .map_err(|err| ReaderCoreError::Message(err.to_string()))?;
        Ok(result)
    }

    async fn list_legado_sources(&self) -> Result<Vec<BookSourceMeta>, ReaderCoreError> {
        let source_dir = self.legado_source_dir.to_string_lossy().to_string();
        let mut out = Vec::new();
        let mut cursor: Option<(i64, String)> = None;
        loop {
            let rows = self
                .source_service
                .list_rows_page_after(
                    USER_NS,
                    SOURCE_LIST_DB_PAGE_SIZE,
                    cursor
                        .as_ref()
                        .map(|(updated_at, url)| (*updated_at, url.as_str())),
                )
                .await?;
            if rows.is_empty() {
                break;
            }
            let row_count = rows.len();
            cursor = rows
                .last()
                .map(|row| (row.updated_at, row.book_source_url.clone()));
            out.extend(
                rows.iter()
                    .filter_map(|row| BookSourceMeta::from_legado_row(row, source_dir.clone())),
            );
            if row_count < SOURCE_LIST_DB_PAGE_SIZE {
                break;
            }
            tokio::task::yield_now().await;
        }
        Ok(out)
    }

    async fn list_js_sources(&self) -> Result<Vec<BookSourceMeta>, ReaderCoreError> {
        let mut out = Vec::new();
        for dir in self.js_source_dirs().await? {
            out.extend(self.list_js_sources_in_dir(&dir).await?);
        }
        Ok(out)
    }

    async fn list_js_sources_in_dir(
        &self,
        dir: &Path,
    ) -> Result<Vec<BookSourceMeta>, ReaderCoreError> {
        let mut out = Vec::new();
        let mut entries = match fs::read_dir(dir).await {
            Ok(entries) => entries,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(out),
            Err(err) => return Err(err.into()),
        };
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().and_then(|value| value.to_str()) != Some("js") {
                continue;
            }
            let file_name = entry.file_name().to_string_lossy().to_string();
            let content = fs::read_to_string(&path).await.unwrap_or_default();
            let metadata = entry.metadata().await.ok();
            if let Some(metadata) = metadata.as_ref() {
                self.store_source_text_cache(&path, &content, metadata)
                    .await;
            }
            out.push(BookSourceMeta::from_js(
                &content,
                file_name,
                dir.to_string_lossy().to_string(),
                metadata.as_ref(),
            ));
        }
        Ok(out)
    }

    async fn persist_legado_source(
        &self,
        file_name: &str,
        source: &BookSource,
    ) -> Result<(), ReaderCoreError> {
        self.persist_legado_source_without_cache_invalidation(file_name, source)
            .await?;
        self.invalidate_source_list_cache().await;
        Ok(())
    }

    async fn persist_legado_source_without_cache_invalidation(
        &self,
        file_name: &str,
        source: &BookSource,
    ) -> Result<(), ReaderCoreError> {
        self.write_legado_source_file(file_name, source).await?;
        self.source_service.save(USER_NS, source.clone()).await?;
        Ok(())
    }

    async fn write_legado_source_file(
        &self,
        file_name: &str,
        source: &BookSource,
    ) -> Result<(), ReaderCoreError> {
        ensure_safe_file_name(file_name)?;
        let path = self.legado_source_dir.join(file_name);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).await?;
        }
        let json = serde_json::to_string_pretty(source)?;
        fs::write(&path, &json).await?;
        self.remove_source_text_cache(&path).await;
        let metadata = fs::metadata(&path).await.ok();
        self.store_legado_source_cache(file_name, source, metadata.as_ref())
            .await;
        Ok(())
    }

    async fn require_legado_source(&self, file_name: &str) -> Result<BookSource, ReaderCoreError> {
        self.get_legado_source_by_file(file_name)
            .await?
            .ok_or_else(|| ReaderCoreError::Message(format!("未找到 Legado 书源: {file_name}")))
    }

    async fn require_js_runtime(
        &self,
        file_name: &str,
        source_dir: Option<&str>,
    ) -> Result<JsSourceRuntime, ReaderCoreError> {
        let content = self.read_source(file_name, source_dir).await?;
        Ok(JsSourceRuntime::new(file_name, content))
    }

    async fn get_legado_source_by_file(
        &self,
        file_name: &str,
    ) -> Result<Option<BookSource>, ReaderCoreError> {
        let path = self.legado_source_dir.join(file_name);
        let metadata = fs::metadata(&path).await.ok();
        if let Some(source) = self
            .cached_legado_source(file_name, metadata.as_ref())
            .await
        {
            return Ok(Some(source));
        }
        if metadata.is_some() {
            let content = fs::read_to_string(&path).await?;
            let value = serde_json::from_str::<Value>(&content)?;
            let source = book_source_from_value(value)?;
            self.store_legado_source_cache(file_name, &source, metadata.as_ref())
                .await;
            return Ok(Some(source));
        }

        let sources = self.source_service.list(USER_NS).await?;
        let source = sources
            .into_iter()
            .find(|source| legado_file_name(source) == file_name);
        if let Some(source) = source.as_ref() {
            self.store_legado_source_cache(file_name, source, None)
                .await;
        } else {
            self.remove_legado_source_cache(file_name).await;
        }
        Ok(source)
    }

    async fn sync_now_inner(
        &self,
        mode: &str,
        domains: Option<Vec<String>>,
        conflict_strategy: Option<&str>,
    ) -> Result<SyncRunSummary, ReaderCoreError> {
        let mode = if mode.trim().is_empty() { "sync" } else { mode };
        if !matches!(mode, "sync" | "pull" | "push") {
            return Err(ReaderCoreError::Message(format!(
                "不支持的同步模式: {mode}"
            )));
        }
        let config = self.sync_config().await?;
        if !config.enabled {
            return Err(ReaderCoreError::Message("同步未启用".to_string()));
        }
        if config.provider != "webdav" {
            return Err(ReaderCoreError::Message(format!(
                "当前只实现 WebDAV 同步，未实现 provider={}",
                config.provider
            )));
        }
        if !config.deferred_domains.is_empty() {
            return Err(ReaderCoreError::Message(format!(
                "以下同步范围尚未实现，请先关闭后重试: {}",
                config.deferred_domains.join(", ")
            )));
        }
        let domains = self.selected_sync_domains(domains, &config)?;
        if domains.is_empty() {
            return Ok(SyncRunSummary {
                status: "success".to_string(),
                mode: mode.to_string(),
                domains,
                uploaded_domains: Vec::new(),
                applied_domains: Vec::new(),
                conflict_count: 0,
                message: "没有启用同步范围".to_string(),
                client_states: Vec::new(),
            });
        }

        let client = self.sync_webdav_client(None).await?;
        let mut uploaded = Vec::new();
        let mut applied = Vec::new();
        let mut conflicts = Vec::new();

        for domain in &domains {
            match mode {
                "push" => {
                    let local = self.sync_domain_snapshot(domain).await?;
                    client.put_domain(domain, &local).await?;
                    uploaded.push(domain.clone());
                }
                "pull" => {
                    if let Some(remote) = client.get_domain(domain).await? {
                        if self.apply_sync_domain(domain, remote).await?.is_some() {
                            // Emitted by the command layer after this returns.
                        }
                        applied.push(domain.clone());
                    }
                }
                "sync" => {
                    let local = self.sync_domain_snapshot(domain).await?;
                    match client.get_domain(domain).await? {
                        None => {
                            client.put_domain(domain, &local).await?;
                            uploaded.push(domain.clone());
                        }
                        Some(remote) if remote == local => {}
                        Some(remote) => match conflict_strategy {
                            Some("local") => {
                                client.put_domain(domain, &local).await?;
                                uploaded.push(domain.clone());
                            }
                            Some("remote") => {
                                self.apply_sync_domain(domain, remote).await?;
                                applied.push(domain.clone());
                            }
                            Some(other) => {
                                return Err(ReaderCoreError::Message(format!(
                                    "不支持的冲突策略: {other}"
                                )));
                            }
                            None => {
                                conflicts.push(SyncConflict {
                                    id: md5_hex(&format!("{domain}:{}", now_ms())),
                                    domain: domain.clone(),
                                    key: format!("{domain}.json"),
                                    message: "本地与远端内容不一致，需要选择保留本地或服务器版本"
                                        .to_string(),
                                    local,
                                    remote,
                                    created_at: now_ms(),
                                    resolved: false,
                                });
                            }
                        },
                    }
                }
                _ => unreachable!(),
            }
        }

        self.sync_runtime.replace_conflicts(conflicts.clone()).await;
        let client_states = self.sync_runtime.client_states_for_domains(&applied).await;
        if !conflicts.is_empty() {
            return Ok(SyncRunSummary {
                status: "conflict".to_string(),
                mode: mode.to_string(),
                domains,
                uploaded_domains: uploaded,
                applied_domains: applied,
                conflict_count: conflicts.len(),
                message: format!("发现 {} 个同步冲突", conflicts.len()),
                client_states,
            });
        }
        Ok(SyncRunSummary {
            status: "success".to_string(),
            mode: mode.to_string(),
            domains,
            uploaded_domains: uploaded.clone(),
            applied_domains: applied.clone(),
            conflict_count: 0,
            message: format!(
                "同步完成：上传 {} 个域，应用 {} 个域",
                uploaded.len(),
                applied.len()
            ),
            client_states,
        })
    }

    async fn sync_webdav_client(
        &self,
        password_override: Option<&str>,
    ) -> Result<WebDavClient, ReaderCoreError> {
        let config = self.sync_config().await?;
        if config.provider != "webdav" {
            return Err(ReaderCoreError::Message(format!(
                "当前只实现 WebDAV 同步，未实现 provider={}",
                config.provider
            )));
        }
        let password = match password_override {
            Some(value) => value.to_string(),
            None => self.read_sync_password().await?.unwrap_or_default(),
        };
        if !config.username.trim().is_empty() && password.is_empty() {
            return Err(ReaderCoreError::Message(
                "WebDAV 密码/Token 未保存".to_string(),
            ));
        }
        WebDavClient::new(self.book_service.http_client().clone(), config, password)
    }

    async fn sync_config(&self) -> Result<WebDavConfig, ReaderCoreError> {
        let config = self.app_config_get_all().await?;
        let provider = config_string_value(&config, "sync_provider", "webdav");
        let enabled_domains = SYNC_SUPPORTED_DOMAINS
            .iter()
            .filter(|domain| config_bool_value(&config, &format!("sync_scope_{domain}"), false))
            .map(|domain| (*domain).to_string())
            .collect::<Vec<_>>();
        let deferred_domains = SYNC_DEFERRED_DOMAINS
            .iter()
            .filter(|domain| config_bool_value(&config, &format!("sync_scope_{domain}"), false))
            .map(|domain| (*domain).to_string())
            .collect::<Vec<_>>();
        Ok(WebDavConfig {
            enabled: config_bool_value(&config, "sync_enabled", false),
            provider,
            base_url: config_string_value(&config, "sync_webdav_url", ""),
            username: config_string_value(&config, "sync_webdav_username", ""),
            root_dir: config_string_value(&config, "sync_webdav_root_dir", "legado-sync"),
            allow_http: config_bool_value(&config, "sync_webdav_allow_http", false),
            enabled_domains,
            deferred_domains,
        })
    }

    fn selected_sync_domains(
        &self,
        requested: Option<Vec<String>>,
        config: &WebDavConfig,
    ) -> Result<Vec<String>, ReaderCoreError> {
        let mut domains = requested.unwrap_or_else(|| config.enabled_domains.clone());
        domains.retain(|item| !item.trim().is_empty());
        dedupe_strings(&mut domains);
        for domain in &domains {
            if SYNC_DEFERRED_DOMAINS.contains(&domain.as_str()) {
                return Err(ReaderCoreError::Message(format!(
                    "同步域 {domain} 尚未实现"
                )));
            }
            if !SYNC_SUPPORTED_DOMAINS.contains(&domain.as_str()) {
                return Err(ReaderCoreError::Message(format!("未知同步域: {domain}")));
            }
        }
        Ok(domains)
    }

    async fn sync_domain_snapshot(&self, domain: &str) -> Result<Value, ReaderCoreError> {
        match domain {
            "bookshelf" => self.bookshelf_snapshot().await,
            "reading_progress" => self.reading_progress_snapshot().await,
            "booksources" => self.booksources_snapshot().await,
            "app_settings" => self.app_config_get_all().await,
            "reader_settings" | "source_flags" => Ok(self
                .sync_runtime
                .client_state(domain)
                .await
                .unwrap_or(Value::Null)),
            other => Err(ReaderCoreError::Message(format!("同步域 {other} 尚未实现"))),
        }
    }

    async fn apply_sync_domain(
        &self,
        domain: &str,
        value: Value,
    ) -> Result<Option<SyncClientState>, ReaderCoreError> {
        match domain {
            "bookshelf" => {
                self.apply_bookshelf_snapshot(value).await?;
                Ok(None)
            }
            "reading_progress" => {
                self.apply_reading_progress_snapshot(value).await?;
                Ok(None)
            }
            "booksources" => {
                self.apply_booksources_snapshot(value).await?;
                Ok(None)
            }
            "app_settings" => {
                self.apply_app_settings_snapshot(value).await?;
                Ok(None)
            }
            "reader_settings" | "source_flags" => {
                self.sync_runtime
                    .set_client_state(domain, value.clone())
                    .await;
                Ok(Some(SyncClientState {
                    domain: domain.to_string(),
                    value,
                }))
            }
            other => Err(ReaderCoreError::Message(format!("同步域 {other} 尚未实现"))),
        }
    }

    async fn bookshelf_snapshot(&self) -> Result<Value, ReaderCoreError> {
        let books = self
            .read_shelf_books()
            .await?
            .into_values()
            .collect::<Vec<_>>();
        Ok(json!({
            "schemaVersion": 1,
            "books": books,
        }))
    }

    async fn apply_bookshelf_snapshot(&self, value: Value) -> Result<(), ReaderCoreError> {
        let books = value
            .get("books")
            .cloned()
            .ok_or_else(|| ReaderCoreError::Message("远端书架数据缺少 books 字段".to_string()))?;
        let items = serde_json::from_value::<Vec<ShelfBook>>(books)?;
        let map = items
            .into_iter()
            .map(|book| (book.id.clone(), book))
            .collect::<BTreeMap<_, _>>();
        self.write_shelf_books(&map).await
    }

    async fn reading_progress_snapshot(&self) -> Result<Value, ReaderCoreError> {
        let books = self.read_shelf_books().await?;
        let progress = books
            .values()
            .map(|book| {
                json!({
                    "id": book.id,
                    "readChapterIndex": book.read_chapter_index,
                    "readChapterUrl": book.read_chapter_url,
                    "readPageIndex": book.read_page_index,
                    "readScrollRatio": book.read_scroll_ratio,
                    "readPlaybackTime": book.read_playback_time,
                    "readerSettings": book.reader_settings,
                    "lastReadAt": book.last_read_at,
                })
            })
            .collect::<Vec<_>>();
        Ok(json!({
            "schemaVersion": 1,
            "books": progress,
        }))
    }

    async fn apply_reading_progress_snapshot(&self, value: Value) -> Result<(), ReaderCoreError> {
        let Some(items) = value.get("books").and_then(|v| v.as_array()) else {
            return Err(ReaderCoreError::Message(
                "远端阅读进度数据缺少 books 字段".to_string(),
            ));
        };
        let mut books = self.read_shelf_books().await?;
        for item in items {
            let Some(id) = item.get("id").and_then(|v| v.as_str()) else {
                continue;
            };
            let Some(book) = books.get_mut(id) else {
                continue;
            };
            if let Some(value) = item.get("readChapterIndex").and_then(|v| v.as_i64()) {
                book.read_chapter_index = value as i32;
            }
            if item.get("readChapterUrl").is_some() {
                book.read_chapter_url = item
                    .get("readChapterUrl")
                    .and_then(|v| v.as_str())
                    .map(str::to_string);
            }
            if let Some(value) = item.get("readPageIndex").and_then(|v| v.as_i64()) {
                book.read_page_index = value as i32;
            }
            if let Some(value) = item.get("readScrollRatio").and_then(|v| v.as_f64()) {
                book.read_scroll_ratio = value;
            }
            if let Some(value) = item.get("readPlaybackTime").and_then(|v| v.as_f64()) {
                book.read_playback_time = value;
            }
            if item.get("readerSettings").is_some() {
                book.reader_settings = item
                    .get("readerSettings")
                    .and_then(|v| v.as_str())
                    .map(str::to_string);
            }
            if let Some(value) = item.get("lastReadAt").and_then(|v| v.as_i64()) {
                book.last_read_at = value;
            }
        }
        self.write_shelf_books(&books).await
    }

    async fn booksources_snapshot(&self) -> Result<Value, ReaderCoreError> {
        Ok(json!({
            "schemaVersion": 1,
            "js": self.source_file_snapshot(&self.js_source_dir, "js").await?,
            "legado": self.source_file_snapshot(&self.legado_source_dir, "legado.json").await?,
        }))
    }

    async fn source_file_snapshot(
        &self,
        dir: &Path,
        suffix: &str,
    ) -> Result<Vec<Value>, ReaderCoreError> {
        let mut out = Vec::new();
        let mut entries = match fs::read_dir(dir).await {
            Ok(entries) => entries,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(out),
            Err(err) => return Err(err.into()),
        };
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let file_name = entry.file_name().to_string_lossy().to_string();
            if !file_name.ends_with(suffix) {
                continue;
            }
            out.push(json!({
                "fileName": file_name,
                "content": fs::read_to_string(path).await.unwrap_or_default(),
            }));
        }
        Ok(out)
    }

    async fn apply_booksources_snapshot(&self, value: Value) -> Result<(), ReaderCoreError> {
        self.write_source_bundle(value.get("js"), &self.js_source_dir, ".js")
            .await?;
        self.write_source_bundle(value.get("legado"), &self.legado_source_dir, ".legado.json")
            .await
    }

    async fn write_source_bundle(
        &self,
        value: Option<&Value>,
        dir: &Path,
        required_suffix: &str,
    ) -> Result<(), ReaderCoreError> {
        let Some(items) = value.and_then(|v| v.as_array()) else {
            return Ok(());
        };
        fs::create_dir_all(dir).await?;
        for item in items {
            let Some(file_name) = item.get("fileName").and_then(|v| v.as_str()) else {
                continue;
            };
            ensure_safe_file_name(file_name)?;
            if !file_name.ends_with(required_suffix) {
                return Err(ReaderCoreError::Message(format!(
                    "远端书源文件扩展名不匹配: {file_name}"
                )));
            }
            let content = item
                .get("content")
                .and_then(|v| v.as_str())
                .unwrap_or_default();
            fs::write(dir.join(file_name), content).await?;
        }
        Ok(())
    }

    async fn apply_app_settings_snapshot(&self, value: Value) -> Result<(), ReaderCoreError> {
        let Some(object) = value.as_object() else {
            return Err(ReaderCoreError::Message(
                "远端 app settings 不是 JSON 对象".to_string(),
            ));
        };
        for (key, value) in object {
            self.config_write_json(APP_CONFIG_SCOPE, key, value).await?;
        }
        Ok(())
    }

    fn sync_credentials_path(&self) -> PathBuf {
        self.reader_dir
            .join("config")
            .join("sync-webdav-credentials.json")
    }

    async fn read_sync_password(&self) -> Result<Option<String>, ReaderCoreError> {
        let path = self.sync_credentials_path();
        let raw = match fs::read_to_string(path).await {
            Ok(raw) => raw,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(err) => return Err(err.into()),
        };
        let value: Value = serde_json::from_str(&raw)?;
        Ok(value
            .get("webdavPassword")
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .filter(|v| !v.is_empty()))
    }

    fn resolve_source_file(&self, file_name: &str, source_dir: Option<&str>) -> PathBuf {
        let fallback = if file_name.ends_with(".legado.json") {
            self.legado_source_dir.clone()
        } else {
            self.js_source_dir.clone()
        };
        let base = source_dir
            .filter(|value| !value.trim().is_empty())
            .map(PathBuf::from)
            .unwrap_or(fallback);
        base.join(file_name)
    }

    fn is_legado_file(&self, file_name: &str, source_dir: Option<&str>) -> bool {
        file_name.ends_with(".legado.json")
            || source_dir
                .map(|value| value.contains(LEGADO_SOURCE_DIR_LABEL))
                .unwrap_or(false)
    }

    fn shelf_path(&self) -> PathBuf {
        self.reader_dir
            .join("data")
            .join(USER_NS)
            .join("shelf.json")
    }

    fn shelf_book_dir(&self, id: &str) -> PathBuf {
        self.reader_dir
            .join("data")
            .join(USER_NS)
            .join("shelf")
            .join(safe_storage_name(id))
    }

    fn content_path(&self, id: &str, chapter_index: i32) -> PathBuf {
        self.shelf_book_dir(id)
            .join("content")
            .join(format!("{chapter_index}.txt"))
    }

    async fn read_shelf_books(&self) -> Result<BTreeMap<String, ShelfBook>, ReaderCoreError> {
        self.read_json_file(&self.shelf_path()).await
    }

    async fn write_shelf_books(
        &self,
        books: &BTreeMap<String, ShelfBook>,
    ) -> Result<(), ReaderCoreError> {
        self.write_json_file(&self.shelf_path(), books).await
    }

    async fn read_json_file<T>(&self, path: &Path) -> Result<T, ReaderCoreError>
    where
        T: serde::de::DeserializeOwned + Default,
    {
        match fs::read_to_string(path).await {
            Ok(raw) => Ok(serde_json::from_str(&raw)?),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(T::default()),
            Err(err) => Err(err.into()),
        }
    }

    async fn write_json_file<T: Serialize + ?Sized>(
        &self,
        path: &Path,
        value: &T,
    ) -> Result<(), ReaderCoreError> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).await?;
        }
        fs::write(path, serde_json::to_string_pretty(value)?).await?;
        Ok(())
    }

    async fn external_source_dirs(&self) -> Result<Vec<String>, ReaderCoreError> {
        let Some(value) = self
            .config_read_json(SOURCE_DIRS_CONFIG_SCOPE, SOURCE_DIRS_CONFIG_KEY)
            .await?
        else {
            return Ok(Vec::new());
        };
        Ok(value
            .as_array()
            .map(|items| {
                items
                    .iter()
                    .filter_map(|item| item.as_str())
                    .map(str::trim)
                    .filter(|item| !item.is_empty())
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default())
    }

    async fn save_external_source_dirs(&self, dirs: &[String]) -> Result<(), ReaderCoreError> {
        self.config_write_json(
            SOURCE_DIRS_CONFIG_SCOPE,
            SOURCE_DIRS_CONFIG_KEY,
            &serde_json::to_value(dirs)?,
        )
        .await
    }

    async fn js_source_dirs(&self) -> Result<Vec<PathBuf>, ReaderCoreError> {
        let mut dirs = vec![self.js_source_dir.clone()];
        dirs.extend(
            self.external_source_dirs()
                .await?
                .into_iter()
                .map(PathBuf::from),
        );
        dedupe_paths(&mut dirs);
        Ok(dirs)
    }

    /// 调试：导出存储状态摘要
    pub async fn debug_dump(&self) -> Result<Value, ReaderCoreError> {
        let frontend_ns = self.frontend_storage_list_namespaces().await?;
        let mut frontend_map = serde_json::Map::new();
        for ns in &frontend_ns {
            let entries = self.frontend_storage_list(&ns.namespace).await?;
            let mut total_size = 0usize;
            for e in &entries {
                total_size += e.value.len();
            }
            frontend_map.insert(
                ns.namespace.clone(),
                serde_json::json!({
                    "count": ns.count,
                    "totalValueBytes": total_size,
                }),
            );
        }

        let app_config = self.app_config_get_all().await.unwrap_or_default();
        let shelf_books = self.read_shelf_books().await?;
        let shelf_count = shelf_books.len();

        Ok(serde_json::json!({
            "frontend": frontend_map,
            "frontendNamespaceCount": frontend_ns.len(),
            "appConfigKeys": app_config.as_object().map(|o| o.len()).unwrap_or(0),
            "shelfBookCount": shelf_count,
            "appStatePath": self.reader_dir.to_string_lossy(),
            "bookshelfPath": self.shelf_path().to_string_lossy(),
            "dbPath": self.reader_dir.join("reader.db").to_string_lossy(),
        }))
    }

    /// 安全解析书源文件绝对路径。仅允许在已知 source 目录下查找文件。
    pub fn resolve_source_path(
        &self,
        file_name: &str,
        source_dir: Option<&str>,
    ) -> Result<PathBuf, ReaderCoreError> {
        ensure_safe_file_name(file_name)?;
        if let Some(dir) = source_dir {
            if !self.legado_source_dir.to_string_lossy().contains(dir)
                && !dir.contains("reader")
                && !dir.contains("sources")
            {
                // 外部目录——需确认在已注册 external 目录内
                return Err(ReaderCoreError::Message(
                    "外部 source_dir 尚未在配置中校验".to_string(),
                ));
            }
        }
        let resolved = self.resolve_source_file(file_name, source_dir);
        // 验证最终路径在 reader 数据目录下或已知允许目录
        if !resolved.starts_with(&self.reader_dir) {
            let allowed = resolved.starts_with(&self.js_source_dir)
                || resolved.starts_with(&self.legado_source_dir);
            if !allowed {
                return Err(ReaderCoreError::Message("文件路径超出允许范围".to_string()));
            }
        }
        Ok(resolved)
    }

    /// HTTP 代理请求（受限）
    pub async fn http_proxy_request(
        &self,
        url: &str,
        method: &str,
        body: Option<&str>,
        headers: Option<&[String]>,
    ) -> Result<String, ReaderCoreError> {
        let m = if method.is_empty() { "GET" } else { method };
        let client = self.book_service.http_client();
        let method = reqwest::Method::from_bytes(m.as_bytes())
            .map_err(|e| ReaderCoreError::Message(format!("无效 HTTP 方法: {e}")))?;
        let is_body_allowed = method != reqwest::Method::GET && method != reqwest::Method::HEAD;
        let mut req = client.request(method, url);
        if let Some(hdrs) = headers {
            for h in hdrs {
                if let Some((k, v)) = h.split_once(':') {
                    req = req.header(k.trim(), v.trim());
                }
            }
        }
        if let Some(b) = body {
            if is_body_allowed {
                req = req.body(b.to_string());
            }
        }
        let fut = req.timeout(std::time::Duration::from_secs(35));
        Ok(fut.send().await?.text().await?)
    }

    /// AI 模型代理请求（白名单路径，避免前端 CORS 问题）。
    pub async fn ai_proxy_request(
        &self,
        url: &str,
        method: &str,
        body: Option<&str>,
        headers: Option<&[String]>,
    ) -> Result<AiHttpProxyResponse, ReaderCoreError> {
        let target = validate_ai_proxy_url(url).map_err(ReaderCoreError::Message)?;
        let m = if method.is_empty() { "POST" } else { method };
        let method = reqwest::Method::from_bytes(m.as_bytes())
            .map_err(|e| ReaderCoreError::Message(format!("无效 HTTP 方法: {e}")))?;
        if method != reqwest::Method::POST {
            return Err(ReaderCoreError::Message(
                "AI HTTP 代理仅支持 POST 请求".to_string(),
            ));
        }

        let client = self.book_service.http_client();
        let mut req = client.request(method, target);
        if let Some(hdrs) = headers {
            for h in hdrs {
                if let Some((raw_key, raw_value)) = h.split_once(':') {
                    let key = raw_key.trim();
                    if key.eq_ignore_ascii_case("host")
                        || key.eq_ignore_ascii_case("content-length")
                        || key.eq_ignore_ascii_case("connection")
                    {
                        continue;
                    }
                    req = req.header(key, raw_value.trim());
                }
            }
        }
        if let Some(b) = body {
            req = req.body(b.to_string());
        }

        let response = req.timeout(ai_proxy_timeout()).send().await?;
        let status = response.status().as_u16();
        let headers = response
            .headers()
            .iter()
            .filter_map(|(name, value)| {
                if name.as_str().eq_ignore_ascii_case("set-cookie") {
                    return None;
                }
                value
                    .to_str()
                    .ok()
                    .map(|value| format!("{}: {}", name.as_str(), value))
            })
            .collect();
        let body = response.text().await?;
        Ok(AiHttpProxyResponse {
            status,
            headers,
            body,
        })
    }

    /// 删除书源草稿
    pub async fn delete_draft(&self, file_name: &str) -> Result<(), ReaderCoreError> {
        ensure_safe_file_name(file_name)?;
        let path = self.reader_dir.join("drafts").join(file_name);
        if path.exists() {
            fs::remove_file(&path).await?;
        }
        Ok(())
    }

    /// 导出书籍（返回文件路径）
    pub async fn export_book(
        &self,
        id: &str,
        format: &str,
        save_path: &str,
    ) -> Result<(), ReaderCoreError> {
        let book = self.shelf_get(id).await?;
        let chapters = self.shelf_get_chapters(id).await?;
        if chapters.is_empty() {
            return Err(ReaderCoreError::Message("没有章节数据可导出".to_string()));
        }

        let raw_path = Path::new(save_path);
        let ext = raw_path
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or("txt");
        let actual_format = if format.is_empty() || format == "auto" {
            ext.to_lowercase()
        } else {
            format.to_lowercase()
        };

        let header = format!(
            "{}\n作者：{}\n来源：{}\n\n",
            book.name, book.author, book.source_name
        );

        let content: String = match actual_format.as_str() {
            "txt" => {
                let mut body = header;
                let mut missing = 0;
                for ch in &chapters {
                    let text = self.shelf_get_content(id, ch.index).await?;
                    body.push_str(&format!(
                        "\n\n第{}章：{}\n\n{}",
                        ch.index + 1,
                        ch.name,
                        text.as_deref().unwrap_or("[未缓存]")
                    ));
                    if text.is_none() {
                        missing += 1;
                    }
                }
                if missing > 0 {
                    // 仍然写入，但部分章节缺失
                }
                body
            }
            "json" => {
                let mut map = serde_json::Map::new();
                map.insert("book".to_string(), serde_json::to_value(&book)?);
                let mut chapter_list = Vec::new();
                let mut contents = serde_json::Map::new();
                for ch in &chapters {
                    let text = self.shelf_get_content(id, ch.index).await?;
                    chapter_list.push(serde_json::to_value(ch)?);
                    contents.insert(
                        ch.index.to_string(),
                        serde_json::to_value(text.as_deref().unwrap_or(""))?,
                    );
                }
                map.insert(
                    "chapters".to_string(),
                    serde_json::Value::Array(chapter_list),
                );
                map.insert("contents".to_string(), serde_json::Value::Object(contents));
                map.insert("exportedAt".to_string(), serde_json::to_value(now_ts())?);
                map.insert(
                    "schemaVersion".to_string(),
                    serde_json::Value::Number(1.into()),
                );
                serde_json::to_string_pretty(&map)?
            }
            _ => {
                return Err(ReaderCoreError::Message(format!(
                    "不支持的导出格式: {format}。仅支持 txt / json"
                )))
            }
        };

        if let Some(parent) = raw_path.parent() {
            fs::create_dir_all(parent).await?;
        }
        fs::write(raw_path, &content).await?;
        Ok(())
    }

    /// 导出书籍数据（返回 base64 编码，用于移动端）
    pub async fn export_book_data(&self, id: &str, format: &str) -> Result<Value, ReaderCoreError> {
        let book = self.shelf_get(id).await?;
        let chapters = self.shelf_get_chapters(id).await?;
        if chapters.is_empty() {
            return Err(ReaderCoreError::Message("没有章节数据可导出".to_string()));
        }

        let format = if format.is_empty() { "txt" } else { format };
        let ext = if format == "json" { "json" } else { "txt" };
        let header = format!(
            "{}\n作者：{}\n来源：{}\n\n",
            book.name, book.author, book.source_name
        );

        let body: String = match format {
            "txt" => {
                let mut body = header;
                for ch in &chapters {
                    let text = self.shelf_get_content(id, ch.index).await?;
                    body.push_str(&format!(
                        "\n\n第{}章：{}\n\n{}",
                        ch.index + 1,
                        ch.name,
                        text.as_deref().unwrap_or("[未缓存]")
                    ));
                }
                body
            }
            "json" => {
                let mut map = serde_json::Map::new();
                map.insert("book".to_string(), serde_json::to_value(&book)?);
                let mut chapter_list = Vec::new();
                let mut contents = serde_json::Map::new();
                for ch in &chapters {
                    let text = self.shelf_get_content(id, ch.index).await?;
                    chapter_list.push(serde_json::to_value(ch)?);
                    contents.insert(
                        ch.index.to_string(),
                        serde_json::to_value(text.as_deref().unwrap_or(""))?,
                    );
                }
                map.insert(
                    "chapters".to_string(),
                    serde_json::Value::Array(chapter_list),
                );
                map.insert("contents".to_string(), serde_json::Value::Object(contents));
                map.insert("exportedAt".to_string(), serde_json::to_value(now_ts())?);
                map.insert(
                    "schemaVersion".to_string(),
                    serde_json::Value::Number(1.into()),
                );
                serde_json::to_string_pretty(&map)?
            }
            _ => {
                return Err(ReaderCoreError::Message(format!(
                    "不支持的导出格式: {format}。仅支持 txt / json"
                )))
            }
        };

        use base64::Engine;
        let b64 = base64::engine::general_purpose::STANDARD.encode(body.as_bytes());
        Ok(serde_json::json!({
            "fileName": format!("{}.{}", book.name, ext),
            "mime": if format == "json" { "application/json" } else { "text/plain; charset=utf-8" },
            "base64": b64,
        }))
    }
}

impl From<SearchBook> for BookItem {
    fn from(value: SearchBook) -> Self {
        Self {
            name: value.name,
            author: value.author,
            book_url: value.book_url,
            cover_url: value.cover_url,
            last_chapter: value.last_chapter.clone(),
            latest_chapter: value.last_chapter,
            latest_chapter_url: None,
            word_count: value.word_count,
            chapter_count: None,
            update_time: value.update_time,
            status: None,
            kind: value.kind,
            intro: value.intro,
        }
    }
}

impl From<Book> for BookDetail {
    fn from(value: Book) -> Self {
        Self {
            name: value.name,
            author: value.author,
            book_url: Some(value.book_url),
            cover_url: value.cover_url,
            intro: value.intro,
            kind: value.kind,
            last_chapter: value.latest_chapter_title.clone(),
            latest_chapter: value.latest_chapter_title,
            latest_chapter_url: None,
            word_count: value.word_count,
            chapter_count: value.total_chapter_num,
            update_time: value.update_time,
            status: None,
            toc_url: value.toc_url,
        }
    }
}

impl From<BookChapter> for ChapterItem {
    fn from(value: BookChapter) -> Self {
        Self {
            name: value.title,
            url: value.url,
            group: value.tag,
            vip: Some(value.is_vip || value.is_pay),
            is_vip: Some(value.is_vip || value.is_pay),
            price: None,
            currency: None,
        }
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct LegadoSourceMetaSeed {
    book_source_name: String,
    book_source_group: Option<String>,
    book_source_url: String,
    #[serde(deserialize_with = "deserialize_meta_i32_option")]
    book_source_type: Option<i32>,
    enabled: Option<bool>,
    #[serde(deserialize_with = "deserialize_meta_i64_option")]
    last_update_time: Option<i64>,
    concurrent_rate: Option<String>,
    enable: Option<bool>,
    explore_url: Option<String>,
    rule_find_url: Option<String>,
    rule_search_url: Option<String>,
    rule_search: Option<IgnoredAny>,
    rule_search_list: Option<IgnoredAny>,
    rule_book_info: Option<IgnoredAny>,
    rule_book_name: Option<IgnoredAny>,
    rule_book_author: Option<IgnoredAny>,
    rule_introduce: Option<IgnoredAny>,
    rule_book_intro: Option<IgnoredAny>,
    rule_chapter_url: Option<IgnoredAny>,
    rule_toc: Option<IgnoredAny>,
    rule_chapter_list: Option<IgnoredAny>,
    rule_content: Option<IgnoredAny>,
    rule_book_content: Option<IgnoredAny>,
    rule_explore: Option<IgnoredAny>,
    rule_find_list: Option<IgnoredAny>,
    book_source_comment: Option<String>,
    search_url: Option<String>,
}

impl BookSourceMeta {
    fn from_legado_row(row: &BookSourceListRow, source_dir: String) -> Option<Self> {
        let seed = legado_meta_seed_from_json(&row.json)?;
        let source_url = if seed.book_source_url.trim().is_empty() {
            row.book_source_url.clone()
        } else {
            seed.book_source_url.clone()
        };
        let source_name = if seed.book_source_name.trim().is_empty() {
            row.book_source_name.clone()
        } else {
            seed.book_source_name.clone()
        };
        if source_name.trim().is_empty() || source_url.trim().is_empty() {
            return None;
        }

        let file_name = legado_file_name_parts(&source_name, &source_url);
        let capabilities = legado_seed_capabilities(&seed);
        let tags = seed
            .book_source_group
            .as_deref()
            .map(split_tags)
            .unwrap_or_default();
        let source_key = format!("{source_dir}::{file_name}");
        Some(Self {
            source_key,
            uuid: md5_hex(&source_url),
            file_name,
            name: source_name,
            url: source_url.clone(),
            urls: vec![source_url.clone()],
            homepage_url: Some(source_url),
            author: None,
            logo: None,
            description: seed.book_source_comment,
            enabled: seed.enabled.or(seed.enable).unwrap_or(true),
            file_size: row.json.len() as u64,
            modified_at: row.updated_at.saturating_mul(1000),
            source_dir,
            source_type: match seed.book_source_type.unwrap_or(0) {
                1 => "audio".to_string(),
                _ => "novel".to_string(),
            },
            version: seed
                .last_update_time
                .map(|value| value.to_string())
                .unwrap_or_else(|| "1.0.0".to_string()),
            update_url: None,
            tags,
            min_delay_ms: seed
                .concurrent_rate
                .as_deref()
                .and_then(|value| value.parse::<u64>().ok())
                .unwrap_or(0),
            require_urls: Vec::new(),
            has_explore: Some(capabilities.iter().any(|item| item == "explore")),
            capabilities,
            runtime: SourceRuntimeKind::LegadoRule,
        })
    }

    fn from_js(
        content: &str,
        file_name: String,
        source_dir: String,
        metadata: Option<&std::fs::Metadata>,
    ) -> Self {
        let name = read_js_meta_values(content, "@name")
            .into_iter()
            .next()
            .unwrap_or_else(|| file_name.trim_end_matches(".js").to_string());
        let urls = read_js_meta_values(content, "@url");
        let url = urls.first().cloned().unwrap_or_default();
        let source_type = read_js_meta_values(content, "@type")
            .into_iter()
            .next()
            .unwrap_or_else(|| "novel".to_string());
        let enabled = read_js_meta_values(content, "@enabled")
            .into_iter()
            .next()
            .map(|value| value != "false" && value != "0")
            .unwrap_or(true);
        let capabilities = js_capabilities(content);
        let source_key = format!("{source_dir}::{file_name}");
        Self {
            source_key,
            uuid: md5_hex(&format!("{source_dir}/{file_name}")),
            file_name,
            name,
            url,
            urls,
            homepage_url: read_js_meta_values(content, "@homepage").into_iter().next(),
            author: read_js_meta_values(content, "@author").into_iter().next(),
            logo: read_js_meta_values(content, "@logo").into_iter().next(),
            description: read_js_meta_values(content, "@description")
                .join("\n")
                .into(),
            enabled,
            file_size: metadata.map(|item| item.len()).unwrap_or_default(),
            modified_at: metadata
                .and_then(|item| item.modified().ok())
                .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|duration| duration.as_millis() as i64)
                .unwrap_or_else(now_ms),
            source_dir,
            source_type,
            version: read_js_meta_values(content, "@version")
                .into_iter()
                .next()
                .unwrap_or_else(|| "1.0.0".to_string()),
            update_url: read_js_meta_values(content, "@updateUrl")
                .into_iter()
                .next(),
            tags: read_js_meta_values(content, "@tags")
                .into_iter()
                .flat_map(|value| split_tags(&value))
                .collect(),
            min_delay_ms: read_js_meta_values(content, "@minDelayMs")
                .into_iter()
                .next()
                .and_then(|value| value.parse::<u64>().ok())
                .unwrap_or(0),
            require_urls: read_js_meta_values(content, "@require"),
            has_explore: Some(capabilities.iter().any(|item| item == "explore")),
            capabilities,
            runtime: SourceRuntimeKind::JsScript,
        }
    }
}

fn legado_file_name(source: &BookSource) -> String {
    legado_file_name_parts(&source.book_source_name, &source.book_source_url)
}

fn legado_file_name_parts(source_name: &str, source_url: &str) -> String {
    let safe_name = source_name
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_') {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>()
        .trim_matches('_')
        .chars()
        .take(48)
        .collect::<String>();
    let prefix = if safe_name.is_empty() {
        "legado".to_string()
    } else {
        safe_name
    };
    format!("{}-{}.legado.json", prefix, &md5_hex(source_url)[..12])
}

fn legado_capabilities(source: &BookSource) -> Vec<String> {
    let mut out = Vec::new();
    if source
        .search_url
        .as_deref()
        .is_some_and(|value| !value.trim().is_empty())
        && source.rule_search.is_some()
    {
        out.push("search".to_string());
    }
    if source.rule_book_info.is_some() {
        out.push("bookInfo".to_string());
    }
    if source.rule_toc.is_some() {
        out.push("toc".to_string());
        out.push("chapterList".to_string());
    }
    if source.rule_content.is_some() {
        out.push("content".to_string());
        out.push("chapterContent".to_string());
    }
    if source
        .explore_url
        .as_deref()
        .is_some_and(|value| !value.trim().is_empty())
        || source.rule_explore.is_some()
    {
        out.push("explore".to_string());
    }
    out
}

fn legado_meta_seed_from_json(raw: &str) -> Option<LegadoSourceMetaSeed> {
    let value = serde_json::from_str::<Value>(raw).ok()?;
    serde_json::from_value(migrate_legacy_book_source_value(value)).ok()
}

fn legado_seed_capabilities(seed: &LegadoSourceMetaSeed) -> Vec<String> {
    let mut out = Vec::new();
    let has_search_url = seed
        .search_url
        .as_deref()
        .or(seed.rule_search_url.as_deref())
        .is_some_and(|value| !value.trim().is_empty());
    if has_search_url && (seed.rule_search.is_some() || seed.rule_search_list.is_some()) {
        out.push("search".to_string());
    }
    if seed.rule_book_info.is_some()
        || seed.rule_book_name.is_some()
        || seed.rule_book_author.is_some()
        || seed.rule_introduce.is_some()
        || seed.rule_book_intro.is_some()
        || seed.rule_chapter_url.is_some()
    {
        out.push("bookInfo".to_string());
    }
    if seed.rule_toc.is_some() || seed.rule_chapter_list.is_some() {
        out.push("toc".to_string());
        out.push("chapterList".to_string());
    }
    if seed.rule_content.is_some() || seed.rule_book_content.is_some() {
        out.push("content".to_string());
        out.push("chapterContent".to_string());
    }
    if seed
        .explore_url
        .as_deref()
        .or(seed.rule_find_url.as_deref())
        .is_some_and(|value| !value.trim().is_empty())
        || seed.rule_explore.is_some()
        || seed.rule_find_list.is_some()
    {
        out.push("explore".to_string());
    }
    out
}

fn deserialize_meta_i64_option<'de, D>(deserializer: D) -> Result<Option<i64>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = Option::<Value>::deserialize(deserializer)?;
    Ok(value.and_then(|value| match value {
        Value::Number(num) => num.as_i64(),
        Value::String(raw) => raw.trim().parse::<i64>().ok(),
        _ => None,
    }))
}

fn deserialize_meta_i32_option<'de, D>(deserializer: D) -> Result<Option<i32>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = Option::<Value>::deserialize(deserializer)?;
    Ok(value.and_then(|value| match value {
        Value::Number(num) => num.as_i64().map(|num| num as i32),
        Value::String(raw) if raw.eq_ignore_ascii_case("AUDIO") => Some(1),
        Value::String(raw) => raw.trim().parse::<i32>().ok(),
        _ => None,
    }))
}

fn js_capabilities(content: &str) -> Vec<String> {
    let mut out = Vec::new();
    if has_js_capability(content, "search") {
        out.push("search".to_string());
    }
    if has_js_capability(content, "bookInfo") {
        out.push("bookInfo".to_string());
    }
    if has_js_capability(content, "toc") || has_js_capability(content, "chapterList") {
        out.push("toc".to_string());
        out.push("chapterList".to_string());
    }
    if has_js_capability(content, "content") || has_js_capability(content, "chapterContent") {
        out.push("content".to_string());
        out.push("chapterContent".to_string());
    }
    if has_js_capability(content, "chapterParagraphCommentCounts") {
        out.push("chapterParagraphCommentCounts".to_string());
    }
    if has_js_capability(content, "chapterParagraphComments") {
        out.push("chapterParagraphComments".to_string());
    }
    if has_js_capability(content, "likeParagraphComment") {
        out.push("likeParagraphComment".to_string());
    }
    if has_js_capability(content, "replyParagraphComment") {
        out.push("replyParagraphComment".to_string());
    }
    if has_js_capability(content, "explore") {
        out.push("explore".to_string());
    }
    out
}

fn has_js_capability(content: &str, name: &str) -> bool {
    let pattern = format!(
        r"(async\s+function\s+{0}\b|function\s+{0}\b|(const|let|var)\s+{0}\s*=)",
        regex::escape(name)
    );
    regex::Regex::new(&pattern)
        .map(|re| re.is_match(content))
        .unwrap_or(false)
}

fn value_to_js_source_arg(value: Value) -> JsSourceArg {
    match value {
        Value::Null => JsSourceArg::Null,
        Value::Bool(b) => JsSourceArg::Bool(b),
        Value::Number(n) => n
            .as_i64()
            .map(|i| JsSourceArg::Int(i as i32))
            .or_else(|| n.as_f64().map(JsSourceArg::Float))
            .unwrap_or(JsSourceArg::Null),
        Value::String(s) => JsSourceArg::String(s),
        other => JsSourceArg::Json(other),
    }
}

fn legado_browser_action_script(expression: &str) -> Result<String, ReaderCoreError> {
    let expression_json = serde_json::to_string(expression)?;
    Ok(r#"
(function() {
  var __legacyAction = __LEGADO_BROWSER_ACTION__;
  var __browser = null;
  function __text(value) {
    return value === undefined || value === null ? "" : String(value);
  }
  function __json(value) {
    if (value === undefined || value === null) return "";
    if (typeof value === "string") return value;
    try { return JSON.stringify(value); } catch (_) { return String(value); }
  }
  function __capture(kind, url, title, html, script, options) {
    __browser = {
      kind: __text(kind),
      url: __text(url),
      title: __text(title),
      html: __text(html),
      script: __text(script),
      options: __text(options)
    };
    return true;
  }
  java.startBrowser = function(url, title, html) {
    return __capture("startBrowser", url, title, html, "", "");
  };
  java.startBrowserAwait = function(url, title, refetchAfterSuccess, html) {
    __capture("startBrowserAwait", url, title, html, "", JSON.stringify({
      refetchAfterSuccess: !!refetchAfterSuccess
    }));
    return "";
  };
  java.showBrowser = function(url, html, preloadJs, config) {
    return __capture("showBrowser", url, "", html, preloadJs, config);
  };
  java.showReadingBrowser = function(url, title) {
    return __capture("showReadingBrowser", url, title, "", "", "");
  };
  try {
    var __result;
    var __tries = /^\s*showCmt\s*\(/.test(__legacyAction) ? 2 : 1;
    for (var __i = 0; __i < __tries; __i++) {
      __result = (0, eval)(__legacyAction);
      if (__browser) break;
    }
    return JSON.stringify({
      ok: true,
      action: __legacyAction,
      result: __json(__result),
      browser: __browser
    });
  } catch (e) {
    return JSON.stringify({
      ok: false,
      action: __legacyAction,
      error: String(e && (e.stack || e.message) || e)
    });
  }
})()
"#
    .replace("__LEGADO_BROWSER_ACTION__", &expression_json))
}

fn js_join_error(err: tokio::task::JoinError) -> ReaderCoreError {
    ReaderCoreError::Message(format!("JS 书源任务执行失败: {err}"))
}

async fn cancellable_sleep(
    duration: Duration,
    cancel_token: &Arc<AtomicBool>,
) -> Result<(), ReaderCoreError> {
    let mut remaining = duration;
    while remaining > Duration::ZERO {
        if cancel_token.load(Ordering::SeqCst) {
            return Err(ReaderCoreError::Message("任务已取消".to_string()));
        }
        let slice = remaining.min(Duration::from_millis(50));
        tokio::time::sleep(slice).await;
        remaining = remaining.saturating_sub(slice);
    }
    if cancel_token.load(Ordering::SeqCst) {
        return Err(ReaderCoreError::Message("任务已取消".to_string()));
    }
    Ok(())
}

fn read_js_meta_values(content: &str, key: &str) -> Vec<String> {
    content
        .lines()
        .take(80)
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

fn set_js_meta_enabled(content: &str, enabled: bool) -> String {
    let mut found = false;
    let mut lines = Vec::new();
    for line in content.lines() {
        let normalized = line.trim_start().trim_start_matches("//").trim_start();
        if normalized.starts_with("@enabled") {
            found = true;
            lines.push(format!("// @enabled     {enabled}"));
        } else {
            lines.push(line.to_string());
        }
    }
    if !found {
        lines.insert(0, format!("// @enabled     {enabled}"));
    }
    let mut out = lines.join("\n");
    out.push('\n');
    out
}

fn split_tags(raw: &str) -> Vec<String> {
    raw.split(|ch| matches!(ch, ',' | ';' | '；' | '、'))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .collect()
}

/// First value of a JS source meta header (e.g. `@version`).
fn first_js_meta(content: &str, key: &str) -> Option<String> {
    read_js_meta_values(content, key).into_iter().next()
}

/// Heuristic: does this text look like a JS book source (has `@name` or `@url`)?
/// Used to reject HTML error pages / non-source downloads before installing.
fn looks_like_js_source(content: &str) -> bool {
    first_js_meta(content, "@name").is_some() || first_js_meta(content, "@url").is_some()
}

/// Stable identity for a JS source: declared `@uuid` if present, otherwise the
/// path-based id `BookSourceMeta::from_js` uses.
fn source_identity(content: &str, file_name: &str, source_dir: Option<&str>) -> String {
    if let Some(uuid) = first_js_meta(content, "@uuid") {
        return uuid;
    }
    let dir = source_dir.unwrap_or("");
    md5_hex(&format!("{dir}/{file_name}"))
}

/// Derive a `.js` file name from a download URL's last path segment.
fn file_name_from_url(url: &str) -> String {
    let trimmed = url.split(['?', '#']).next().unwrap_or(url);
    let last = trimmed.rsplit('/').next().unwrap_or("").trim();
    if last.is_empty() {
        "remote-source.js".to_string()
    } else if last.to_ascii_lowercase().ends_with(".js") {
        last.to_string()
    } else {
        format!("{last}.js")
    }
}

/// Whether `remote` is a newer version than `local`. Numeric dot-versions
/// compare component-wise; otherwise any non-empty difference counts as update.
fn version_has_update(local: &str, remote: &str) -> bool {
    let remote = remote.trim();
    if remote.is_empty() {
        return false;
    }
    match (parse_dot_version(local), parse_dot_version(remote)) {
        (Some(l), Some(r)) => r > l,
        _ => remote != local.trim(),
    }
}

fn parse_dot_version(value: &str) -> Option<Vec<u64>> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    value
        .split('.')
        .map(|part| part.trim().parse::<u64>().ok())
        .collect()
}

/// Normalize a JS source for content comparison: drop `@enabled` / `@uuid` meta
/// lines (which differ per install) and trailing whitespace, so two copies that
/// differ only in those are considered consistent.
fn normalize_source_for_compare(content: &str) -> String {
    content
        .lines()
        .filter(|line| {
            let normalized = line
                .trim_start()
                .trim_start_matches("//")
                .trim_start_matches('*')
                .trim_start();
            !normalized.starts_with("@enabled") && !normalized.starts_with("@uuid")
        })
        .map(str::trim_end)
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

fn ensure_safe_file_name(file_name: &str) -> Result<(), ReaderCoreError> {
    if file_name.trim().is_empty()
        || file_name.contains("..")
        || file_name
            .chars()
            .any(|ch| matches!(ch, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|'))
    {
        return Err(ReaderCoreError::Message(format!("非法文件名: {file_name}")));
    }
    Ok(())
}

fn normalize_source_dir(value: &str) -> Result<PathBuf, ReaderCoreError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(ReaderCoreError::Message("书源目录不能为空".to_string()));
    }
    let path = PathBuf::from(trimmed);
    if !path.is_absolute() {
        return Err(ReaderCoreError::Message(format!(
            "书源目录必须是绝对路径: {trimmed}"
        )));
    }
    Ok(path)
}

fn dedupe_strings(values: &mut Vec<String>) {
    let mut seen = HashSet::new();
    values.retain(|value| seen.insert(value.clone()));
}

fn dedupe_paths(values: &mut Vec<PathBuf>) {
    let mut seen = HashSet::new();
    values.retain(|value| seen.insert(value.clone()));
}

fn validate_network_url(value: &str) -> Result<(), ReaderCoreError> {
    let parsed = url::Url::parse(value)
        .map_err(|_| ReaderCoreError::Message(format!("URL 格式不正确: {value}")))?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err(ReaderCoreError::Message(format!(
            "不支持的 URL 协议: {}",
            parsed.scheme()
        )));
    }
    Ok(())
}

fn safe_storage_name(value: &str) -> String {
    md5_hex(value)
}

fn shelf_id(book_url: &str, file_name: &str, source_dir: Option<&str>) -> String {
    let source_dir = source_dir.map(str::trim).filter(|value| !value.is_empty());
    match source_dir {
        Some(source_dir) => md5_hex(&format!("{book_url}|{file_name}|{source_dir}")),
        None => md5_hex(&format!("{book_url}|{file_name}")),
    }
}

fn clean_optional_string(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn content_cache_book_key(file_name: &str, chapter_url: &str) -> String {
    format!("{file_name}|{}", chapter_url_host_path_prefix(chapter_url))
}

fn chapter_url_host_path_prefix(chapter_url: &str) -> String {
    url::Url::parse(chapter_url)
        .ok()
        .map(|url| {
            let mut segments = url
                .path_segments()
                .map(|segments| segments.collect::<Vec<_>>())
                .unwrap_or_default();
            let _ = segments.pop();
            format!(
                "{}://{}/{}",
                url.scheme(),
                url.host_str().unwrap_or_default(),
                segments.join("/")
            )
        })
        .unwrap_or_else(|| chapter_url.to_string())
}

fn now_ms() -> i64 {
    now_ts() * 1000
}

fn config_namespace(scope: &str) -> String {
    format!("config:{scope}")
}

fn frontend_namespace(namespace: &str) -> String {
    format!("{FRONTEND_STORAGE_PREFIX}{namespace}")
}

/// Load the merged app config (defaults overlaid with persisted values) using
/// only the pool, so it can run before the full `ReaderCore` is assembled (the
/// HTTP client is built from it at startup).
async fn load_app_config(pool: &SqlitePool) -> Value {
    let mut merged = default_app_config();
    let namespace = config_namespace(APP_CONFIG_SCOPE);
    let rows = sqlx::query("SELECT name, json FROM json_documents WHERE namespace=?1")
        .bind(namespace)
        .fetch_all(pool)
        .await
        .unwrap_or_default();
    if let Some(base) = merged.as_object_mut() {
        for row in rows {
            let key: String = row.get("name");
            let raw: String = row.get("json");
            let value = serde_json::from_str(&raw).unwrap_or(Value::String(raw));
            base.insert(key, value);
        }
    }
    merged
}

/// Read a u64 config value, accepting both JSON numbers and the string-encoded
/// form the settings UI persists. Falls back to `default` when absent/invalid.
fn config_u64_value(value: &Value, key: &str, default: u64) -> u64 {
    value
        .get(key)
        .and_then(|v| {
            v.as_u64()
                .or_else(|| v.as_str().and_then(|s| s.trim().parse::<u64>().ok()))
        })
        .unwrap_or(default)
}

fn config_string_value(value: &Value, key: &str, default: &str) -> String {
    value
        .get(key)
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .unwrap_or_else(|| default.to_string())
}

fn config_bool_value(value: &Value, key: &str, default: bool) -> bool {
    match value.get(key) {
        Some(Value::Bool(value)) => *value,
        Some(Value::String(value)) => match value.trim().to_ascii_lowercase().as_str() {
            "true" | "1" | "yes" | "on" => true,
            "false" | "0" | "no" | "off" | "" => false,
            _ => default,
        },
        Some(Value::Number(value)) => value.as_i64().map(|v| v != 0).unwrap_or(default),
        _ => default,
    }
}

fn sanitize_sync_error(value: &str) -> String {
    // Keep protocol/status detail but avoid echoing long request internals that
    // may contain Basic auth material in lower-level error messages.
    value
        .lines()
        .next()
        .unwrap_or("同步失败")
        .chars()
        .take(240)
        .collect()
}

fn find_progress_for_book(books: &Value, book_id: &str) -> Option<Value> {
    books.as_array()?.iter().find_map(|item| {
        if item.get("id").and_then(|v| v.as_str()) == Some(book_id) {
            Some(item.clone())
        } else {
            None
        }
    })
}

fn default_app_config() -> Value {
    json!({
        "http_user_agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
        "http_follow_redirects": true,
        "http_connect_timeout_secs": 10,
        "http_ignore_tls_errors": true,
        "http_doh_server": "none",
        "proxy_mode": "system",
        "proxy_type": "http",
        "proxy_host": "",
        "proxy_port": 0,
        "proxy_username": "",
        "proxy_password": "",
        "engine_timeout_secs": 30,
        "booksource_watcher_enabled": false,
        "browser_probe_enabled": true,
        "browser_probe_user_agent": "",
        "browser_probe_timeout_secs": 0,
        "browser_probe_visible_by_default": false,
        "browser_probe_force_visible": false,
        "browser_probe_persist_profile": true,
        "comic_cache_enabled": true,
        "ui_layout_mode": "auto",
        "ui_theme": "auto",
        "ui_theme_color": "",
        "power_keep_awake_on_tts": false,
        "power_reader_awake_mode": "off",
        "power_reader_awake_timeout_secs": 600,
        "windows_main_window_width": 0,
        "windows_main_window_height": 0,
        "video_player_type": "xgplayer",
        "video_default_rate": 1.0,
        "video_auto_next": true,
        "video_quality_prefer": "auto",
        "video_remember_progress": true,
        "video_seek_step_secs": 10,
        "video_vjs_preload": "auto",
        "video_vjs_pip": true,
        "video_xg_download": false,
        "video_dp_danmaku": false,
        "video_dp_theme": "#00b1ff",
        "video_autoplay": true,
        "web_server_enabled": false,
        "web_server_port": 7688,
        "web_server_dist_path": "",
        "web_remote_debug_enabled": false,
        "web_remote_debug_host": "",
        "web_remote_debug_port": 8080,
        "request_min_delay_ms": 300,
        "cache_prefetch_count": 3,
        "cache_prefetch_concurrency": 2,
        "export_prefetch_concurrency": 3,
        "sync_enabled": false,
        "sync_provider": "webdav",
        "sync_profile_id": "default",
        "sync_webdav_url": "",
        "sync_webdav_username": "",
        "sync_webdav_root_dir": "legado-sync",
        "sync_webdav_allow_http": false,
        "sync_trigger_enabled": true,
        "sync_timer_enabled": false,
        "sync_timer_interval_secs": 900,
        "sync_trigger_on_startup": true,
        "sync_trigger_on_resume": true,
        "sync_trigger_on_unlock_resume": true,
        "sync_trigger_on_bookshelf_change": false,
        "sync_trigger_on_booksource_change": false,
        "sync_trigger_on_settings_change": false,
        "sync_scope_bookshelf": true,
        "sync_scope_reading_progress": true,
        "sync_scope_booksources": true,
        "sync_scope_reader_settings": true,
        "sync_scope_app_settings": true,
        "sync_scope_source_flags": false,
        "sync_scope_extensions": false,
        "sync_scope_script_config": false,
        "sync_mobile_foreground_only": true,
        "sync_mobile_screen_on_only": true,
        "sync_mobile_wifi_only": true,
        "sync_mobile_pause_on_low_battery": true,
        "sync_mobile_startup_delay_ms": 5000,
        "sync_mobile_resume_delay_ms": 1500,
        "sync_baidu_app_name": "legado-tauri"
    })
}

#[cfg(test)]
mod cap_repo_tests {
    use super::*;

    #[test]
    fn version_has_update_numeric_and_string() {
        assert!(version_has_update("1.0.0", "1.0.1"));
        assert!(version_has_update("1.0.0", "2.0.0"));
        assert!(version_has_update("1.2", "1.10")); // numeric, not lexical
        assert!(!version_has_update("2.0.0", "1.9.9"));
        assert!(!version_has_update("1.0.0", "1.0.0"));
        assert!(!version_has_update("1.0.0", "")); // empty remote = no update
                                                   // non-numeric versions fall back to plain inequality
        assert!(version_has_update("2026-06-10", "2026-06-12"));
        assert!(!version_has_update("v1", "v1"));
    }

    #[test]
    fn normalize_ignores_enabled_and_uuid_lines() {
        let a = "// @name X\n// @enabled true\n// @uuid aaa\nfunction f(){}\n";
        let b = "// @name X\n// @enabled false\n// @uuid bbb\nfunction f(){}  \n";
        assert_eq!(
            normalize_source_for_compare(a),
            normalize_source_for_compare(b)
        );
        let c = "// @name X\nfunction g(){}\n";
        assert_ne!(
            normalize_source_for_compare(a),
            normalize_source_for_compare(c)
        );
    }

    #[test]
    fn file_name_from_url_derives_js_name() {
        assert_eq!(file_name_from_url("https://x.com/a/demo.js"), "demo.js");
        assert_eq!(file_name_from_url("https://x.com/a/demo.js?v=2"), "demo.js");
        assert_eq!(file_name_from_url("https://x.com/a/demo"), "demo.js");
        assert_eq!(file_name_from_url("https://x.com/"), "remote-source.js");
    }

    #[test]
    fn looks_like_js_source_detects_meta() {
        assert!(looks_like_js_source("// @name Demo\nfn"));
        assert!(looks_like_js_source("// @url https://x\n"));
        assert!(!looks_like_js_source("<html><body>error</body></html>"));
        assert!(!looks_like_js_source("{\"name\":\"repo\"}"));
    }
}
