use crate::model::ai_model::AiModelKind;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::net::IpAddr;
use std::time::Duration;
use url::Url;

const AI_PROXY_TIMEOUT_SECS: u64 = 300;
const ALLOWED_PROXY_PATHS: [&str; 4] = [
    "/v1/chat/completions",
    "/v1/responses",
    "/v1/images/generations",
    "/v1/audio/speech",
];
const ALLOWED_AI_PROXY_HOSTS: [&str; 2] = ["api.deepseek.com", "api.openai.com"];

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiProxyRequest {
    #[serde(default)]
    pub base_url: String,
    pub api_key: Option<String>,
    pub path: String,
    #[serde(default)]
    pub full_url: bool,
    #[serde(default)]
    pub use_server_config: bool,
    pub kind: Option<AiModelKind>,
    pub body: Value,
}

#[derive(Debug, Deserialize)]
pub struct AiProxyImageRequest {
    pub url: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiHttpProxyResponse {
    pub status: u16,
    pub headers: Vec<String>,
    pub body: String,
}

pub fn build_ai_proxy_url(base_url: &str, path: &str, full_url: bool) -> Result<Url, String> {
    if full_url {
        return parse_http_url(base_url);
    }

    if !ALLOWED_PROXY_PATHS.contains(&path) {
        return Err(format!("unsupported proxy path: {}", path));
    }

    let mut base = parse_http_url(base_url)?;
    let joined_path = format!("{}{}", base.path().trim_end_matches('/'), path,);
    base.set_path(&joined_path);
    base.set_query(None);
    base.set_fragment(None);
    Ok(base)
}

pub fn validate_ai_proxy_url(url: &str) -> Result<Url, String> {
    let parsed = parse_http_url(url)?;
    validate_allowed_ai_proxy_host(&parsed)?;
    if !ALLOWED_PROXY_PATHS.contains(&parsed.path()) {
        return Err(format!("unsupported proxy path: {}", parsed.path()));
    }
    Ok(parsed)
}

pub fn validate_ai_proxy_image_url(url: &str) -> Result<Url, String> {
    parse_http_url(url)
}

pub fn ai_proxy_timeout() -> Duration {
    Duration::from_secs(AI_PROXY_TIMEOUT_SECS)
}

pub fn format_ai_proxy_upstream_error(status: u16, body: &str) -> String {
    let reason = http::StatusCode::from_u16(status)
        .ok()
        .and_then(|status| status.canonical_reason())
        .unwrap_or("Upstream Error");
    let detail = extract_error_detail(body);
    if detail.is_empty() {
        return format!("模型服务返回 {} {}", status, reason);
    }
    format!("模型服务返回 {} {}：{}", status, reason, detail)
}

fn parse_http_url(raw: &str) -> Result<Url, String> {
    let url = Url::parse(raw.trim()).map_err(|e| e.to_string())?;
    match url.scheme() {
        "http" | "https" => {
            validate_public_host(&url)?;
            Ok(url)
        }
        _ => Err("only http/https proxy targets are supported".to_string()),
    }
}

fn validate_public_host(url: &Url) -> Result<(), String> {
    let host = url
        .host_str()
        .ok_or_else(|| "proxy target host is required".to_string())?
        .trim_matches(|ch| ch == '[' || ch == ']')
        .to_lowercase();
    if host == "localhost" {
        return Err("localhost proxy targets are blocked".to_string());
    }
    if let Ok(ip) = host.parse::<IpAddr>() {
        let blocked = match ip {
            IpAddr::V4(addr) => {
                addr.is_loopback()
                    || addr.is_private()
                    || addr.is_link_local()
                    || addr.is_unspecified()
            }
            IpAddr::V6(addr) => {
                addr.is_loopback()
                    || addr.is_unique_local()
                    || addr.is_unicast_link_local()
                    || addr.is_unspecified()
            }
        };
        if blocked {
            return Err("private proxy targets are blocked".to_string());
        }
    }
    Ok(())
}

fn validate_allowed_ai_proxy_host(url: &Url) -> Result<(), String> {
    let host = url
        .host_str()
        .ok_or_else(|| "proxy target host is required".to_string())?
        .trim_matches(|ch| ch == '[' || ch == ']')
        .to_lowercase();
    if ALLOWED_AI_PROXY_HOSTS.contains(&host.as_str()) {
        return Ok(());
    }
    Err(format!("unsupported AI proxy host: {host}"))
}

fn extract_error_detail(body: &str) -> String {
    if let Ok(value) = serde_json::from_str::<Value>(body) {
        if let Some(message) = value
            .pointer("/error/message")
            .or_else(|| value.get("errorMsg"))
            .or_else(|| value.get("message"))
            .and_then(Value::as_str)
        {
            return truncate_error_detail(message);
        }
    }

    truncate_error_detail(&strip_html_tags(body))
}

fn strip_html_tags(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    let mut in_tag = false;
    for ch in value.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => {
                in_tag = false;
                output.push(' ');
            }
            _ if !in_tag => output.push(ch),
            _ => {}
        }
    }
    output.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn truncate_error_detail(value: &str) -> String {
    let cleaned = value.trim();
    if cleaned.chars().count() <= 240 {
        return cleaned.to_string();
    }
    let mut result = cleaned.chars().take(240).collect::<String>();
    result.push('…');
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_allowed_deepseek_chat_path() {
        let url = validate_ai_proxy_url("https://api.deepseek.com/v1/chat/completions").unwrap();
        assert_eq!(url.host_str(), Some("api.deepseek.com"));
    }

    #[test]
    fn rejects_unlisted_ai_proxy_path() {
        let err = validate_ai_proxy_url("https://api.deepseek.com/v1/models").unwrap_err();
        assert!(err.contains("unsupported proxy path"));
    }

    #[test]
    fn rejects_local_ai_proxy_target() {
        let err = validate_ai_proxy_url("http://127.0.0.1/v1/chat/completions").unwrap_err();
        assert!(err.contains("blocked"));
    }

    #[test]
    fn rejects_unknown_ai_proxy_host() {
        let err = validate_ai_proxy_url("https://example.com/v1/chat/completions").unwrap_err();
        assert!(err.contains("unsupported AI proxy host"));
    }
}
