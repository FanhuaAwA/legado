use crate::app_state::ReaderCoreOptions;
use crate::crawler::http_client::HttpClient;
use crate::dto::{
    AddBookPayload, BookDetail, BookItem, BookSourceMeta, CachedChapter, ChapterItem,
    EpisodeProgress, EpisodeProgressMap, FrontendStorageEntry, FrontendStorageNamespaceSummary,
    LegacyJsonImportResult, ShelfBook, SourceRuntimeKind, SourceSwitchRestoreResult,
    UpdateShelfBookPayload,
};
use crate::error::ReaderCoreError;
use crate::model::article_source::ArticleSource;
use crate::model::book::Book;
use crate::model::book_chapter::BookChapter;
use crate::model::book_source::{book_source_from_value, BookSource};
use crate::model::search::SearchBook;
use crate::parser::js::JsSourceArg;
use crate::parser::rule_engine::RuleEngine;
use crate::service::book_service::BookService;
use crate::service::book_source_service::BookSourceService;
use crate::service::json_document_service::JsonDocumentService;
use crate::source_runtime::js_source::JsSourceRuntime;
use crate::storage::cache::file_cache::FileCache;
use crate::storage::db::init_pool;
use crate::util::hash::md5_hex;
use crate::util::time::now_ts;
use serde::Serialize;
use serde_json::{json, Value};
use sqlx::{Row, SqlitePool};
use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};
use tokio::fs;

const USER_NS: &str = "local";
const LEGADO_SOURCE_DIR_LABEL: &str = "legado-json";
const FRONTEND_STORAGE_PREFIX: &str = "frontend:";
const APP_CONFIG_SCOPE: &str = "app.config";
const SOURCE_DIRS_CONFIG_SCOPE: &str = "booksource.dirs";
const SOURCE_DIRS_CONFIG_KEY: &str = "external";

#[derive(Clone)]
pub struct ReaderCore {
    reader_dir: PathBuf,
    js_source_dir: PathBuf,
    legado_source_dir: PathBuf,
    pool: SqlitePool,
    source_service: BookSourceService,
    book_service: BookService,
    document_service: JsonDocumentService,
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
        let http = HttpClient::new(options.request_timeout_secs.max(1), None)?;
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
        self.save_external_source_dirs(&dirs).await
    }

    pub async fn remove_source_dir(&self, dir_path: &str) -> Result<(), ReaderCoreError> {
        let path = normalize_source_dir(dir_path)?;
        let target = path.to_string_lossy().to_string();
        let mut dirs = self.external_source_dirs().await?;
        dirs.retain(|dir| dir != &target);
        self.save_external_source_dirs(&dirs).await
    }

    pub async fn list_sources(&self) -> Result<Vec<BookSourceMeta>, ReaderCoreError> {
        let mut out = Vec::new();
        out.extend(self.list_js_sources().await?);
        out.extend(self.list_legado_sources().await?);
        out.extend(self.list_article_sources().await?);
        out.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(out)
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
                    runtime: SourceRuntimeKind::LegacyArticle,
                });
            }
        }
        Ok(out)
    }

    pub async fn read_source(
        &self,
        file_name: &str,
        source_dir: Option<&str>,
    ) -> Result<String, ReaderCoreError> {
        let path = self.resolve_source_file(file_name, source_dir);
        fs::read_to_string(path)
            .await
            .map_err(ReaderCoreError::from)
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
        fs::write(path, content).await?;
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
        match fs::remove_file(path).await {
            Ok(()) => Ok(()),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
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
        let content = fs::read_to_string(&path).await?;
        let content = set_js_meta_enabled(&content, enabled);
        fs::write(path, content).await?;
        Ok(())
    }

    pub async fn import_legacy_json_text(
        &self,
        content: &str,
        _smart_explore_sub_categories: bool,
    ) -> Result<LegacyJsonImportResult, ReaderCoreError> {
        let value: Value = serde_json::from_str(content)?;
        let values = match value {
            Value::Array(items) => items,
            other => vec![other],
        };

        let mut result = LegacyJsonImportResult {
            imported: 0,
            skipped: 0,
            files: Vec::new(),
            errors: Vec::new(),
        };
        let mut seen = HashSet::new();

        for value in values {
            // Try as article source first (sourceName + ruleArticles)
            if let (Some(name), Some(_rules)) = (
                value.get("sourceName").and_then(|v| v.as_str()),
                value.get("ruleArticles"),
            ) {
                if !name.trim().is_empty() {
                    let article: ArticleSource = serde_json::from_value(value.clone())?;
                    let file_name = format!(
                        "{}.article.json",
                        article.source_name.replace(['/', '\\', ':', '?', '*', '"', '<', '>', '|'], "_")
                    );
                    let article_dir = self.reader_dir.join("sources").join("article-json");
                    fs::create_dir_all(&article_dir).await?;
                    let article_path = article_dir.join(&file_name);
                    fs::write(&article_path, serde_json::to_string_pretty(&article)?).await?;
                    result.imported += 1;
                    result.files.push(file_name);
                    continue;
                }
            }

            match book_source_from_value(value) {
                Ok(source) => {
                    if source.book_source_name.trim().is_empty()
                        || source.book_source_url.trim().is_empty()
                    {
                        result.skipped += 1;
                        result
                            .errors
                            .push("缺少 bookSourceName 或 bookSourceUrl".to_string());
                        continue;
                    }
                    if !seen.insert(source.book_source_url.clone()) {
                        result.skipped += 1;
                        continue;
                    }
                    let file_name = legado_file_name(&source);
                    self.persist_legado_source(&file_name, &source).await?;
                    result.imported += 1;
                    result.files.push(file_name);
                }
                Err(err) => {
                    result.skipped += 1;
                    result.errors.push(err.to_string());
                }
            }
        }

        Ok(result)
    }

    pub async fn import_legacy_json_url(
        &self,
        url: &str,
        smart_explore_sub_categories: bool,
    ) -> Result<LegacyJsonImportResult, ReaderCoreError> {
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
        self.import_legacy_json_text(&text, smart_explore_sub_categories)
            .await
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

    pub async fn save_draft(
        &self,
        file_name: &str,
        content: &str,
    ) -> Result<(), ReaderCoreError> {
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
        let _ = timeout_secs;
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

            for step_name in &enabled {
                let start = std::time::Instant::now();
                match step_name.as_str() {
                    "search" => {
                        if source.rule_search.is_none() {
                            steps.push(TestStep {
                                name: "search".into(), status: "skipped".into(),
                                elapsed_ms: 0, error: Some("ruleSearch 未配置".into()),
                                sample_count: None, output_preview: None,
                            });
                            continue;
                        }
                        match self.search(file_name, "测试", 1, source_dir).await {
                            Ok(items) => {
                                let preview = items.first().map(|i| format!("{} - {}", i.name, i.author));
                                steps.push(TestStep {
                                    name: "search".into(), status: "passed".into(),
                                    elapsed_ms: start.elapsed().as_millis() as u64,
                                    error: None, sample_count: Some(items.len()),
                                    output_preview: preview,
                                });
                            }
                            Err(e) => {
                                steps.push(TestStep {
                                    name: "search".into(), status: "failed".into(),
                                    elapsed_ms: start.elapsed().as_millis() as u64,
                                    error: Some(e.to_string()), sample_count: None,
                                    output_preview: None,
                                });
                            }
                        }
                    }
                    "bookInfo" => {
                        if source.rule_book_info.is_none() {
                            steps.push(TestStep {
                                name: "bookInfo".into(), status: "skipped".into(),
                                elapsed_ms: 0, error: Some("ruleBookInfo 未配置".into()),
                                sample_count: None, output_preview: None,
                            });
                            continue;
                        }
                        let dummy_url = if source.book_source_url.is_empty() {
                            "https://example.com".to_string()
                        } else {
                            source.book_source_url.clone()
                        };
                        match self.book_info(file_name, &dummy_url, source_dir).await {
                            Ok(detail) => {
                                steps.push(TestStep {
                                    name: "bookInfo".into(), status: "passed".into(),
                                    elapsed_ms: start.elapsed().as_millis() as u64,
                                    error: None, sample_count: None,
                                    output_preview: Some(format!("{} - {}", detail.name, detail.author)),
                                });
                            }
                            Err(e) => {
                                steps.push(TestStep {
                                    name: "bookInfo".into(), status: "failed".into(),
                                    elapsed_ms: start.elapsed().as_millis() as u64,
                                    error: Some(e.to_string()), sample_count: None,
                                    output_preview: None,
                                });
                            }
                        }
                    }
                    "toc" => {
                        if source.rule_toc.is_none() {
                            steps.push(TestStep {
                                name: "toc".into(), status: "skipped".into(),
                                elapsed_ms: 0, error: Some("ruleToc 未配置".into()),
                                sample_count: None, output_preview: None,
                            });
                            continue;
                        }
                        let dummy_url = if source.book_source_url.is_empty() {
                            "https://example.com".to_string()
                        } else {
                            source.book_source_url.clone()
                        };
                        match self.chapter_list(file_name, &dummy_url, source_dir).await {
                            Ok(chapters) => {
                                let count = chapters.len();
                                let preview = chapters.first().map(|c| c.name.clone());
                                steps.push(TestStep {
                                    name: "toc".into(), status: "passed".into(),
                                    elapsed_ms: start.elapsed().as_millis() as u64,
                                    error: None, sample_count: Some(count),
                                    output_preview: preview,
                                });
                            }
                            Err(e) => {
                                steps.push(TestStep {
                                    name: "toc".into(), status: "failed".into(),
                                    elapsed_ms: start.elapsed().as_millis() as u64,
                                    error: Some(e.to_string()), sample_count: None,
                                    output_preview: None,
                                });
                            }
                        }
                    }
                    "content" => {
                        if source.rule_content.is_none() {
                            steps.push(TestStep {
                                name: "content".into(), status: "skipped".into(),
                                elapsed_ms: 0, error: Some("ruleContent 未配置".into()),
                                sample_count: None, output_preview: None,
                            });
                            continue;
                        }
                        let dummy_url = if source.book_source_url.is_empty() {
                            "https://example.com".to_string()
                        } else {
                            source.book_source_url.clone()
                        };
                        match self.chapter_content(file_name, &dummy_url, source_dir).await {
                            Ok(text) => {
                                let trimmed: String = text.chars().take(100).collect();
                                steps.push(TestStep {
                                    name: "content".into(), status: "passed".into(),
                                    elapsed_ms: start.elapsed().as_millis() as u64,
                                    error: None, sample_count: Some(text.len()),
                                    output_preview: Some(trimmed),
                                });
                            }
                            Err(e) => {
                                steps.push(TestStep {
                                    name: "content".into(), status: "failed".into(),
                                    elapsed_ms: start.elapsed().as_millis() as u64,
                                    error: Some(e.to_string()), sample_count: None,
                                    output_preview: None,
                                });
                            }
                        }
                    }
                    "explore" => {
                        if source.rule_explore.is_none() {
                            steps.push(TestStep {
                                name: "explore".into(), status: "skipped".into(),
                                elapsed_ms: 0, error: Some("ruleExplore 未配置".into()),
                                sample_count: None, output_preview: None,
                            });
                            continue;
                        }
                        match self.explore(file_name, 1, "", source_dir).await {
                            Ok(result) => {
                                steps.push(TestStep {
                                    name: "explore".into(), status: "passed".into(),
                                    elapsed_ms: start.elapsed().as_millis() as u64,
                                    error: None, sample_count: None,
                                    output_preview: Some(serde_json::to_string(&result).unwrap_or_default().chars().take(200).collect()),
                                });
                            }
                            Err(e) => {
                                steps.push(TestStep {
                                    name: "explore".into(), status: "failed".into(),
                                    elapsed_ms: start.elapsed().as_millis() as u64,
                                    error: Some(e.to_string()), sample_count: None,
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
                "search" => {
                    match runtime.search("test", 1) {
                        Ok(r) => {
                            let items = serde_json::to_value(r).unwrap_or_default();
                            let count = items.as_array().map(|a| a.len()).unwrap_or(0);
                            steps.push(TestStep {
                                name: "search".into(), status: "passed".into(),
                                elapsed_ms: start.elapsed().as_millis() as u64,
                                error: None, sample_count: Some(count),
                                output_preview: Some(format!("{} results", count)),
                            });
                        }
                        Err(e) => steps.push(TestStep {
                            name: "search".into(), status: "failed".into(),
                            elapsed_ms: start.elapsed().as_millis() as u64,
                            error: Some(e.to_string()), sample_count: None, output_preview: None,
                        }),
                    }
                }
                "bookInfo" => {
                    match runtime.book_info("https://example.com") {
                        Ok(r) => {
                            let v = serde_json::to_value(r).unwrap_or_default();
                            steps.push(TestStep {
                                name: "bookInfo".into(), status: "passed".into(),
                                elapsed_ms: start.elapsed().as_millis() as u64,
                                error: None, sample_count: None,
                                output_preview: Some(serde_json::to_string(&v).unwrap_or_default().chars().take(200).collect()),
                            });
                        }
                        Err(e) => steps.push(TestStep {
                            name: "bookInfo".into(), status: "failed".into(),
                            elapsed_ms: start.elapsed().as_millis() as u64,
                            error: Some(e.to_string()), sample_count: None, output_preview: None,
                        }),
                    }
                }
                "toc" => {
                    match runtime.chapter_list("https://example.com") {
                        Ok(r) => {
                            let v = serde_json::to_value(r).unwrap_or_default();
                            let count = v.as_array().map(|a| a.len()).unwrap_or(0);
                            steps.push(TestStep {
                                name: "toc".into(), status: "passed".into(),
                                elapsed_ms: start.elapsed().as_millis() as u64,
                                error: None, sample_count: Some(count),
                                output_preview: Some(format!("{} chapters", count)),
                            });
                        }
                        Err(e) => steps.push(TestStep {
                            name: "toc".into(), status: "failed".into(),
                            elapsed_ms: start.elapsed().as_millis() as u64,
                            error: Some(e.to_string()), sample_count: None, output_preview: None,
                        }),
                    }
                }
                "content" => {
                    match runtime.chapter_content("https://example.com") {
                        Ok(r) => {
                            steps.push(TestStep {
                                name: "content".into(), status: "passed".into(),
                                elapsed_ms: start.elapsed().as_millis() as u64,
                                error: None, sample_count: Some(r.len()),
                                output_preview: Some(r.chars().take(100).collect()),
                            });
                        }
                        Err(e) => steps.push(TestStep {
                            name: "content".into(), status: "failed".into(),
                            elapsed_ms: start.elapsed().as_millis() as u64,
                            error: Some(e.to_string()), sample_count: None, output_preview: None,
                        }),
                    }
                }
                "explore" => {
                    match runtime.explore(1, "") {
                        Ok(r) => steps.push(TestStep {
                            name: "explore".into(), status: "passed".into(),
                            elapsed_ms: start.elapsed().as_millis() as u64,
                            error: None, sample_count: None,
                            output_preview: Some(serde_json::to_string(&r).unwrap_or_default().chars().take(200).collect()),
                        }),
                        Err(e) => steps.push(TestStep {
                            name: "explore".into(), status: "failed".into(),
                            elapsed_ms: start.elapsed().as_millis() as u64,
                            error: Some(e.to_string()), sample_count: None, output_preview: None,
                        }),
                    }
                }
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
        if !self.is_legado_file(file_name, source_dir) {
            let runtime = self.require_js_runtime(file_name, source_dir).await?;
            let keyword = keyword.to_string();
            return tokio::task::spawn_blocking(move || runtime.search(&keyword, page))
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
        if !self.is_legado_file(file_name, source_dir) {
            let runtime = self.require_js_runtime(file_name, source_dir).await?;
            let book_url = book_url.to_string();
            return tokio::task::spawn_blocking(move || runtime.chapter_list(&book_url))
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
        if !self.is_legado_file(file_name, source_dir) {
            let runtime = self.require_js_runtime(file_name, source_dir).await?;
            let chapter_url = chapter_url.to_string();
            return tokio::task::spawn_blocking(move || runtime.chapter_content(&chapter_url))
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
        Ok(json!({ "ok": false, "purchased": false, "unsupported": true, "message": "Legado 规则书源不支持自动购买，请手动处理" }))
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
        Err(ReaderCoreError::Message(format!(
            "Legado 规则书源不支持自定义 JS 函数调用: {fn_name}"
        )))
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

    pub async fn prefetch_chapters(
        &self,
        id: &str,
        file_name: &str,
        source_dir: Option<&str>,
        cancel_token: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
    ) -> Result<i32, ReaderCoreError> {
        let chapters = self.shelf_get_chapters(id).await?;
        let mut count = 0;
        let mut errors = 0usize;
        let max_retries = 2usize;
        for chapter in &chapters {
            if let Some(ref token) = cancel_token {
                if token.load(std::sync::atomic::Ordering::SeqCst) {
                    return Err(ReaderCoreError::Message("任务已取消".to_string()));
                }
            }
            if chapter.url.is_empty() {
                continue;
            }
            let chapter_idx = chapter.index;
            if self.shelf_get_content(id, chapter_idx).await?.is_some() {
                continue;
            }
            let mut content = Err(ReaderCoreError::Message("未尝试".into()));
            for _ in 0..=max_retries {
                match self
                    .chapter_content(file_name, &chapter.url, source_dir)
                    .await
                {
                    Ok(c) => {
                        content = Ok(c);
                        break;
                    }
                    Err(e) => {
                        tracing::warn!(
                            "prefetch retry {}/{} for chapter {}: {e}",
                            errors,
                            max_retries,
                            chapter_idx
                        );
                        errors += 1;
                    }
                }
            }
            let content = content?;
            self.shelf_save_content(id, chapter_idx, &content).await?;
            count += 1;
        }
        Ok(count)
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
    pub async fn export_book_epub(
        &self,
        id: &str,
        save_path: &str,
    ) -> Result<(), ReaderCoreError> {
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
        let raw = self.config_read_all(APP_CONFIG_SCOPE).await?;
        let stored = serde_json::from_str::<Value>(&raw).unwrap_or_else(|_| json!({}));
        let mut merged = default_app_config();
        if let (Some(base), Some(extra)) = (merged.as_object_mut(), stored.as_object()) {
            for (key, value) in extra {
                base.insert(key.clone(), value.clone());
            }
        }
        Ok(merged)
    }

    pub async fn app_config_set(&self, key: &str, value: &Value) -> Result<(), ReaderCoreError> {
        self.config_write_json(APP_CONFIG_SCOPE, key, value).await
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
        let sources = self.source_service.list(USER_NS).await?;
        let mut out = Vec::with_capacity(sources.len());
        for source in sources {
            let file_name = legado_file_name(&source);
            let path = self.legado_source_dir.join(&file_name);
            let metadata = fs::metadata(&path).await.ok();
            out.push(BookSourceMeta::from_legado(
                &source,
                file_name,
                self.legado_source_dir.to_string_lossy().to_string(),
                metadata.as_ref(),
            ));
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
        ensure_safe_file_name(file_name)?;
        let path = self.legado_source_dir.join(file_name);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).await?;
        }
        let json = serde_json::to_string_pretty(source)?;
        fs::write(path, json).await?;
        self.source_service.save(USER_NS, source.clone()).await?;
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
        if path.exists() {
            let content = fs::read_to_string(path).await?;
            let value = serde_json::from_str::<Value>(&content)?;
            return Ok(Some(book_source_from_value(value)?));
        }

        let sources = self.source_service.list(USER_NS).await?;
        Ok(sources
            .into_iter()
            .find(|source| legado_file_name(source) == file_name))
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
            frontend_map.insert(ns.namespace.clone(), serde_json::json!({
                "count": ns.count,
                "totalValueBytes": total_size,
            }));
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
                return Err(ReaderCoreError::Message(
                    "文件路径超出允许范围".to_string(),
                ));
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
            return Err(ReaderCoreError::Message(
                "没有章节数据可导出".to_string(),
            ));
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

        let header =
            format!("{}\n作者：{}\n来源：{}\n\n", book.name, book.author, book.source_name);

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
                map.insert("chapters".to_string(), serde_json::Value::Array(chapter_list));
                map.insert("contents".to_string(), serde_json::Value::Object(contents));
                map.insert("exportedAt".to_string(), serde_json::to_value(now_ts())?);
                map.insert("schemaVersion".to_string(), serde_json::Value::Number(1.into()));
                serde_json::to_string_pretty(&map)?
            }
            _ => return Err(ReaderCoreError::Message(format!("不支持的导出格式: {format}。仅支持 txt / json"))),
        };

        if let Some(parent) = raw_path.parent() {
            fs::create_dir_all(parent).await?;
        }
        fs::write(raw_path, &content).await?;
        Ok(())
    }

    /// 导出书籍数据（返回 base64 编码，用于移动端）
    pub async fn export_book_data(
        &self,
        id: &str,
        format: &str,
    ) -> Result<Value, ReaderCoreError> {
        let book = self.shelf_get(id).await?;
        let chapters = self.shelf_get_chapters(id).await?;
        if chapters.is_empty() {
            return Err(ReaderCoreError::Message(
                "没有章节数据可导出".to_string(),
            ));
        }

        let format = if format.is_empty() { "txt" } else { format };
        let ext = if format == "json" { "json" } else { "txt" };
        let header =
            format!("{}\n作者：{}\n来源：{}\n\n", book.name, book.author, book.source_name);

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
                map.insert("chapters".to_string(), serde_json::Value::Array(chapter_list));
                map.insert("contents".to_string(), serde_json::Value::Object(contents));
                map.insert("exportedAt".to_string(), serde_json::to_value(now_ts())?);
                map.insert("schemaVersion".to_string(), serde_json::Value::Number(1.into()));
                serde_json::to_string_pretty(&map)?
            }
            _ => return Err(ReaderCoreError::Message(format!("不支持的导出格式: {format}。仅支持 txt / json"))),
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

impl BookSourceMeta {
    fn from_legado(
        source: &BookSource,
        file_name: String,
        source_dir: String,
        metadata: Option<&std::fs::Metadata>,
    ) -> Self {
        let capabilities = legado_capabilities(source);
        let tags = source
            .book_source_group
            .as_deref()
            .map(split_tags)
            .unwrap_or_default();
        let source_key = format!("{source_dir}::{file_name}");
        Self {
            source_key,
            uuid: md5_hex(&source.book_source_url),
            file_name,
            name: source.book_source_name.clone(),
            url: source.book_source_url.clone(),
            urls: vec![source.book_source_url.clone()],
            homepage_url: Some(source.book_source_url.clone()),
            author: None,
            logo: None,
            description: source.book_source_comment.clone(),
            enabled: source.enabled.unwrap_or(true),
            file_size: metadata.map(|item| item.len()).unwrap_or_default(),
            modified_at: metadata
                .and_then(|item| item.modified().ok())
                .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|duration| duration.as_millis() as i64)
                .unwrap_or_else(now_ms),
            source_dir,
            source_type: match source.book_source_type.unwrap_or(0) {
                1 => "audio".to_string(),
                _ => "novel".to_string(),
            },
            version: source
                .last_update_time
                .map(|value| value.to_string())
                .unwrap_or_else(|| "1.0.0".to_string()),
            update_url: None,
            tags,
            min_delay_ms: source
                .concurrent_rate
                .as_deref()
                .and_then(|value| value.parse::<u64>().ok())
                .unwrap_or(0),
            require_urls: Vec::new(),
            has_explore: Some(capabilities.iter().any(|item| item == "explore")),
            runtime: SourceRuntimeKind::LegadoRule,
        }
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
            runtime: SourceRuntimeKind::JsScript,
        }
    }
}

fn legado_file_name(source: &BookSource) -> String {
    let safe_name = source
        .book_source_name
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
    format!(
        "{}-{}.legado.json",
        prefix,
        &md5_hex(&source.book_source_url)[..12]
    )
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

fn js_join_error(err: tokio::task::JoinError) -> ReaderCoreError {
    ReaderCoreError::Message(format!("JS 书源任务执行失败: {err}"))
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
        "ui_enable_aplus_tracking": true,
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
