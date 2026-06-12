use crate::dto::{ReaderSessionPayload, SyncClientState, SyncConflict, SyncStatus};
use crate::error::ReaderCoreError;
use crate::util::time::now_ts;
use reqwest::{Client, Method, StatusCode, Url};
use serde_json::Value;
use std::collections::BTreeMap;
use tokio::sync::Mutex;

#[derive(Debug)]
pub struct SyncRuntime {
    status: Mutex<SyncStatus>,
    conflicts: Mutex<Vec<SyncConflict>>,
    client_states: Mutex<BTreeMap<String, Value>>,
    reader_sessions: Mutex<BTreeMap<String, ReaderSessionPayload>>,
}

impl SyncRuntime {
    pub fn new() -> Self {
        Self {
            status: Mutex::new(SyncStatus::default()),
            conflicts: Mutex::new(Vec::new()),
            client_states: Mutex::new(BTreeMap::new()),
            reader_sessions: Mutex::new(BTreeMap::new()),
        }
    }

    pub async fn status(&self) -> SyncStatus {
        self.status.lock().await.clone()
    }

    pub async fn set_running(&self, running: bool) {
        self.status.lock().await.running = running;
    }

    pub async fn mark_success(&self, summary: impl Into<String>, conflict_count: usize) {
        let mut status = self.status.lock().await;
        status.running = false;
        status.last_success_at = now_ts() * 1000;
        status.last_error.clear();
        status.conflict_count = conflict_count;
        status.last_run_summary = summary.into();
    }

    pub async fn mark_failure(&self, error: impl Into<String>) {
        let mut status = self.status.lock().await;
        status.running = false;
        status.last_failed_at = now_ts() * 1000;
        status.last_error = error.into();
    }

    pub async fn conflicts(&self) -> Vec<SyncConflict> {
        self.conflicts.lock().await.clone()
    }

    pub async fn replace_conflicts(&self, conflicts: Vec<SyncConflict>) {
        let count = conflicts.iter().filter(|item| !item.resolved).count();
        *self.conflicts.lock().await = conflicts;
        self.status.lock().await.conflict_count = count;
    }

    pub async fn resolve_conflict(&self, conflict_id: &str) -> Option<SyncConflict> {
        let mut conflicts = self.conflicts.lock().await;
        let item = conflicts
            .iter_mut()
            .find(|item| item.id == conflict_id && !item.resolved)?;
        item.resolved = true;
        let resolved = item.clone();
        let count = conflicts.iter().filter(|item| !item.resolved).count();
        drop(conflicts);
        self.status.lock().await.conflict_count = count;
        Some(resolved)
    }

    pub async fn set_client_state(&self, domain: &str, value: Value) {
        self.client_states
            .lock()
            .await
            .insert(domain.to_string(), value);
    }

    pub async fn client_state(&self, domain: &str) -> Option<Value> {
        self.client_states.lock().await.get(domain).cloned()
    }

    pub async fn client_states_for_domains(&self, domains: &[String]) -> Vec<SyncClientState> {
        let states = self.client_states.lock().await;
        domains
            .iter()
            .filter_map(|domain| {
                states.get(domain).cloned().map(|value| SyncClientState {
                    domain: domain.clone(),
                    value,
                })
            })
            .collect()
    }

    pub async fn set_reader_session(&self, session: ReaderSessionPayload) {
        self.reader_sessions
            .lock()
            .await
            .insert(session.book_id.clone(), session);
    }

    pub async fn reader_session(&self, book_id: &str) -> Option<ReaderSessionPayload> {
        self.reader_sessions.lock().await.get(book_id).cloned()
    }
}

impl Default for SyncRuntime {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
pub struct WebDavConfig {
    pub enabled: bool,
    pub provider: String,
    pub base_url: String,
    pub username: String,
    pub root_dir: String,
    pub allow_http: bool,
    pub enabled_domains: Vec<String>,
    pub deferred_domains: Vec<String>,
}

#[derive(Clone)]
pub struct WebDavClient {
    client: Client,
    config: WebDavConfig,
    password: String,
    root_url: Url,
}

impl WebDavClient {
    pub fn new(
        client: Client,
        config: WebDavConfig,
        password: String,
    ) -> Result<Self, ReaderCoreError> {
        let root_url = build_root_url(&config.base_url, &config.root_dir, config.allow_http)?;
        Ok(Self {
            client,
            config,
            password,
            root_url,
        })
    }

    pub async fn test_connection(&self) -> Result<(), ReaderCoreError> {
        self.ensure_collection(&self.root_url).await?;
        Ok(())
    }

    pub async fn put_domain(&self, domain: &str, value: &Value) -> Result<(), ReaderCoreError> {
        self.ensure_data_collections().await?;
        let url = self.domain_url(domain)?;
        let body = serde_json::to_string_pretty(value)?;
        let res = self
            .request(Method::PUT, url)
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .body(body)
            .send()
            .await?;
        ensure_success(res.status(), "PUT")
    }

    pub async fn get_domain(&self, domain: &str) -> Result<Option<Value>, ReaderCoreError> {
        let url = self.domain_url(domain)?;
        let res = self.request(Method::GET, url).send().await?;
        if res.status() == StatusCode::NOT_FOUND {
            return Ok(None);
        }
        let status = res.status();
        if !status.is_success() {
            return Err(ReaderCoreError::Message(format!(
                "WebDAV GET 失败: HTTP {}",
                status.as_u16()
            )));
        }
        Ok(Some(res.json::<Value>().await?))
    }

    async fn ensure_data_collections(&self) -> Result<(), ReaderCoreError> {
        self.ensure_collection(&self.root_url).await?;
        self.ensure_collection(&self.data_root_url()?).await?;
        Ok(())
    }

    async fn ensure_collection(&self, url: &Url) -> Result<(), ReaderCoreError> {
        if self.propfind_exists(url).await? {
            return Ok(());
        }
        self.mkcol(url).await
    }

    async fn propfind_exists(&self, url: &Url) -> Result<bool, ReaderCoreError> {
        let propfind = Method::from_bytes(b"PROPFIND")
            .map_err(|err| ReaderCoreError::Message(format!("无效 WebDAV 方法: {err}")))?;
        let res = self
            .request(propfind, url.clone())
            .header("Depth", "0")
            .send()
            .await?;
        if res.status() == StatusCode::NOT_FOUND {
            return Ok(false);
        }
        if is_webdav_success(res.status()) {
            return Ok(true);
        }
        Err(ReaderCoreError::Message(format!(
            "WebDAV PROPFIND 失败: HTTP {}",
            res.status().as_u16()
        )))
    }

    async fn mkcol(&self, url: &Url) -> Result<(), ReaderCoreError> {
        let mkcol = Method::from_bytes(b"MKCOL")
            .map_err(|err| ReaderCoreError::Message(format!("无效 WebDAV 方法: {err}")))?;
        let res = self.request(mkcol, url.clone()).send().await?;
        let status = res.status();
        if status.is_success() || status == StatusCode::METHOD_NOT_ALLOWED {
            return Ok(());
        }
        Err(ReaderCoreError::Message(format!(
            "WebDAV MKCOL 失败: HTTP {}",
            status.as_u16()
        )))
    }

    fn request(&self, method: Method, url: Url) -> reqwest::RequestBuilder {
        let req = self.client.request(method, url);
        if self.config.username.trim().is_empty() && self.password.is_empty() {
            req
        } else {
            req.basic_auth(self.config.username.clone(), Some(self.password.clone()))
        }
    }

    fn data_root_url(&self) -> Result<Url, ReaderCoreError> {
        append_segments(&self.root_url, &["legado"], true)
    }

    fn domain_url(&self, domain: &str) -> Result<Url, ReaderCoreError> {
        validate_domain_name(domain)?;
        append_segments(
            &self.root_url,
            &["legado", &format!("{domain}.json")],
            false,
        )
    }
}

fn build_root_url(
    base_url: &str,
    root_dir: &str,
    allow_http: bool,
) -> Result<Url, ReaderCoreError> {
    let trimmed = base_url.trim();
    if trimmed.is_empty() {
        return Err(ReaderCoreError::Message("WebDAV 地址不能为空".to_string()));
    }
    let base = Url::parse(trimmed)
        .map_err(|_| ReaderCoreError::Message("WebDAV 地址格式不正确".to_string()))?;
    match base.scheme() {
        "https" => {}
        "http" if allow_http => {}
        "http" => {
            return Err(ReaderCoreError::Message(
                "当前配置不允许 HTTP WebDAV，请启用“允许 HTTP”或改用 HTTPS".to_string(),
            ))
        }
        other => {
            return Err(ReaderCoreError::Message(format!(
                "不支持的 WebDAV 协议: {other}"
            )))
        }
    }
    let root = normalize_root_dir(root_dir)?;
    let segment_refs = root.iter().map(String::as_str).collect::<Vec<_>>();
    append_segments(&base, &segment_refs, true)
}

fn append_segments(
    base: &Url,
    segments: &[&str],
    trailing_slash: bool,
) -> Result<Url, ReaderCoreError> {
    let mut url = base.clone();
    {
        let mut path = url
            .path_segments_mut()
            .map_err(|_| ReaderCoreError::Message("WebDAV URL 不能作为基础地址".to_string()))?;
        for segment in segments {
            path.push(segment);
        }
    }
    if trailing_slash && !url.as_str().ends_with('/') {
        let mut value = url.to_string();
        value.push('/');
        url = Url::parse(&value)
            .map_err(|_| ReaderCoreError::Message("WebDAV URL 规范化失败".to_string()))?;
    }
    Ok(url)
}

fn normalize_root_dir(root_dir: &str) -> Result<Vec<String>, ReaderCoreError> {
    let raw = if root_dir.trim().is_empty() {
        "legado-sync"
    } else {
        root_dir.trim()
    };
    let mut out = Vec::new();
    for segment in raw.split(['/', '\\']) {
        let segment = segment.trim();
        if segment.is_empty() {
            continue;
        }
        if segment == "." || segment == ".." || segment.contains('\0') {
            return Err(ReaderCoreError::Message(
                "WebDAV 远端目录包含非法路径片段".to_string(),
            ));
        }
        out.push(segment.to_string());
    }
    if out.is_empty() {
        out.push("legado-sync".to_string());
    }
    Ok(out)
}

fn validate_domain_name(domain: &str) -> Result<(), ReaderCoreError> {
    if domain.is_empty()
        || !domain
            .chars()
            .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_')
    {
        return Err(ReaderCoreError::Message(format!(
            "非法同步域名称: {domain}"
        )));
    }
    Ok(())
}

fn is_webdav_success(status: StatusCode) -> bool {
    status.is_success() || status.as_u16() == 207
}

fn ensure_success(status: StatusCode, operation: &str) -> Result<(), ReaderCoreError> {
    if status.is_success() {
        Ok(())
    } else {
        Err(ReaderCoreError::Message(format!(
            "WebDAV {operation} 失败: HTTP {}",
            status.as_u16()
        )))
    }
}
