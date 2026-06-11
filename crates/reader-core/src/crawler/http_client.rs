use reqwest::{Client, ClientBuilder, Proxy};
use serde_json::Value;
use std::time::Duration;

/// Built-in fallback User-Agent, mirrored by `default_app_config()` in facade.rs
/// and `BUILTIN_USER_AGENT` in the Network settings UI.
pub const DEFAULT_USER_AGENT: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";

/// Network settings consumed by the shared HTTP client.
///
/// Parsed from the persisted app config (the `http_*` / `proxy_*` keys of
/// `default_app_config()`). The Network settings UI saves numeric / boolean
/// values as strings, so the parsers below accept both JSON-typed and
/// string-encoded values. Proxy and TLS changes take effect on the next client
/// build; the UI states an app restart is required.
#[derive(Clone, Debug)]
pub struct HttpClientConfig {
    pub request_timeout_secs: u64,
    pub connect_timeout_secs: u64,
    pub user_agent: String,
    pub follow_redirects: bool,
    pub ignore_tls_errors: bool,
    /// "system" | "none" | "custom"
    pub proxy_mode: String,
    /// "http" | "socks5"
    pub proxy_type: String,
    pub proxy_host: String,
    pub proxy_port: u16,
    pub proxy_username: String,
    pub proxy_password: String,
}

impl Default for HttpClientConfig {
    fn default() -> Self {
        Self {
            request_timeout_secs: 30,
            connect_timeout_secs: 10,
            user_agent: DEFAULT_USER_AGENT.to_string(),
            follow_redirects: true,
            ignore_tls_errors: false,
            proxy_mode: "system".to_string(),
            proxy_type: "http".to_string(),
            proxy_host: String::new(),
            proxy_port: 0,
            proxy_username: String::new(),
            proxy_password: String::new(),
        }
    }
}

fn config_str(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(|s| s.to_string())
}

fn config_bool(value: &Value, key: &str) -> Option<bool> {
    match value.get(key) {
        Some(Value::Bool(b)) => Some(*b),
        Some(Value::String(s)) => match s.trim().to_ascii_lowercase().as_str() {
            "true" | "1" | "yes" | "on" => Some(true),
            "false" | "0" | "no" | "off" | "" => Some(false),
            _ => None,
        },
        _ => None,
    }
}

fn config_u64(value: &Value, key: &str) -> Option<u64> {
    match value.get(key) {
        Some(Value::Number(n)) => n.as_u64().or_else(|| n.as_f64().map(|f| f.max(0.0) as u64)),
        Some(Value::String(s)) => s.trim().parse::<u64>().ok(),
        _ => None,
    }
}

impl HttpClientConfig {
    /// Build from a merged app-config object (output of `app_config_get_all`).
    /// `request_timeout_secs` is supplied separately by the caller because the
    /// overall request timeout is a `ReaderCoreOptions` field, not a config key.
    pub fn from_app_config(value: &Value, request_timeout_secs: u64) -> Self {
        let mut cfg = Self {
            request_timeout_secs: request_timeout_secs.max(1),
            ..Self::default()
        };
        if let Some(ua) = config_str(value, "http_user_agent") {
            if !ua.trim().is_empty() {
                cfg.user_agent = ua;
            }
        }
        if let Some(v) = config_bool(value, "http_follow_redirects") {
            cfg.follow_redirects = v;
        }
        if let Some(v) = config_bool(value, "http_ignore_tls_errors") {
            cfg.ignore_tls_errors = v;
        }
        if let Some(v) = config_u64(value, "http_connect_timeout_secs") {
            if v > 0 {
                cfg.connect_timeout_secs = v;
            }
        }
        if let Some(v) = config_str(value, "proxy_mode") {
            cfg.proxy_mode = v;
        }
        if let Some(v) = config_str(value, "proxy_type") {
            cfg.proxy_type = v;
        }
        if let Some(v) = config_str(value, "proxy_host") {
            cfg.proxy_host = v;
        }
        if let Some(v) = config_u64(value, "proxy_port") {
            cfg.proxy_port = v.min(65535) as u16;
        }
        if let Some(v) = config_str(value, "proxy_username") {
            cfg.proxy_username = v;
        }
        if let Some(v) = config_str(value, "proxy_password") {
            cfg.proxy_password = v;
        }
        cfg
    }

    /// Build the custom proxy from host/port/type/credentials. Returns `None`
    /// when host or port is unset (treated as "no custom proxy configured").
    fn custom_proxy(&self) -> anyhow::Result<Option<Proxy>> {
        let host = self.proxy_host.trim();
        if host.is_empty() || self.proxy_port == 0 {
            return Ok(None);
        }
        let scheme = match self.proxy_type.trim().to_ascii_lowercase().as_str() {
            "socks5" | "socks" => "socks5",
            _ => "http",
        };
        let url = format!("{scheme}://{host}:{}", self.proxy_port);
        let mut proxy = Proxy::all(&url)?;
        if !self.proxy_username.is_empty() {
            proxy = proxy.basic_auth(&self.proxy_username, &self.proxy_password);
        }
        Ok(Some(proxy))
    }

    /// Assemble a `ClientBuilder` applying every network setting.
    fn builder(&self) -> anyhow::Result<ClientBuilder> {
        let ua = if self.user_agent.trim().is_empty() {
            DEFAULT_USER_AGENT
        } else {
            self.user_agent.trim()
        };
        let mut builder = Client::builder()
            .timeout(Duration::from_secs(self.request_timeout_secs.max(1)))
            .connect_timeout(Duration::from_secs(self.connect_timeout_secs.max(1)))
            .cookie_store(true)
            .user_agent(ua);

        builder = if self.follow_redirects {
            builder.redirect(reqwest::redirect::Policy::limited(10))
        } else {
            builder.redirect(reqwest::redirect::Policy::none())
        };

        if self.ignore_tls_errors {
            builder = builder.danger_accept_invalid_certs(true);
        }

        match self.proxy_mode.trim().to_ascii_lowercase().as_str() {
            "none" => {
                builder = builder.no_proxy();
            }
            "custom" => {
                if let Some(proxy) = self.custom_proxy()? {
                    builder = builder.proxy(proxy);
                }
            }
            // "system" (default): leave reqwest's default behaviour, which reads
            // standard proxy environment variables / system settings.
            _ => {}
        }

        Ok(builder)
    }
}

#[derive(Clone)]
pub struct HttpClient {
    client: Client,
}

impl HttpClient {
    /// Legacy constructor: default network settings with an explicit timeout and
    /// optional fully-formed proxy URL. Kept for internal/test call sites.
    pub fn new(timeout_secs: u64, proxy: Option<String>) -> anyhow::Result<Self> {
        let cfg = HttpClientConfig {
            request_timeout_secs: timeout_secs.max(1),
            ..HttpClientConfig::default()
        };
        let mut builder = cfg.builder()?;
        if let Some(p) = proxy {
            builder = builder.proxy(Proxy::all(p)?);
        }
        Ok(Self {
            client: builder.build()?,
        })
    }

    /// Build from parsed network config (proxy / UA / TLS / timeouts / redirects).
    pub fn from_config(cfg: &HttpClientConfig) -> anyhow::Result<Self> {
        Ok(Self {
            client: cfg.builder()?.build()?,
        })
    }

    pub fn client(&self) -> &Client {
        &self.client
    }
}
