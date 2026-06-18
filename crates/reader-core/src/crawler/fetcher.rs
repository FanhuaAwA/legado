use crate::crawler::http_client::HttpClient;
use encoding_rs::Encoding;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::OnceLock;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::time::{sleep, Instant};

const SOURCE_FAST_FAIL_TIMEOUT_SECS: u64 = 20;
const SOURCE_FAST_FAIL_MIN_INTERVAL_MS: u64 = 800;
static SOURCE_FAST_FAIL_HOST_STATES: OnceLock<Mutex<HashMap<String, Instant>>> = OnceLock::new();

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum HttpMethod {
    GET,
    POST,
}

impl Default for HttpMethod {
    fn default() -> Self {
        Self::GET
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestSpec {
    pub url: String,
    pub method: HttpMethod,
    pub headers: Vec<(String, String)>,
    pub body: Option<String>,
    pub retry: usize,
    pub response_type: Option<String>,
    pub charset: Option<String>,
    pub proxy: Option<String>,
    pub server_id: Option<i64>,
    pub web_view: bool,
    pub web_js: Option<String>,
    pub web_view_delay_time: u64,
}

impl Default for RequestSpec {
    fn default() -> Self {
        Self {
            url: String::new(),
            method: HttpMethod::GET,
            headers: Vec::new(),
            body: None,
            retry: 2,
            response_type: None,
            charset: None,
            proxy: None,
            server_id: None,
            web_view: false,
            web_js: None,
            web_view_delay_time: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FetchResponse {
    pub url: String,
    pub status: u16,
    pub body: String,
    pub content_type: Option<String>,
    pub headers: Vec<(String, String)>,
    pub is_successful: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct StrResponse {
    pub body: String,
    pub url: String,
    pub code: u16,
    pub headers: Vec<(String, String)>,
    pub is_successful: bool,
}

fn resolve_proxy_url(raw: &str) -> String {
    if let Some(payload) = raw.trim().strip_prefix("data:;base64,") {
        use base64::Engine as _;
        if let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(payload) {
            if let Ok(decoded) = String::from_utf8(bytes) {
                tracing::debug!("fetcher decoded proxy URL: {} -> {}", raw, decoded);
                return decoded;
            }
        }
    }
    raw.to_string()
}

/// Legado-compatible data URI: `data:<mediatype>;base64,<payload>` is not fetched
/// over the network — the decoded payload IS the response body (番茄 tocUrl /
/// chapterUrl carry the book/item id this way). The empty-mediatype form
/// `data:;base64,` is this project's proxy-URL convention handled by
/// `resolve_proxy_url`, so it is excluded here.
pub(crate) fn decode_data_uri(url: &str) -> Option<Vec<u8>> {
    use base64::Engine as _;
    let trimmed = url.trim();
    let rest = trimmed
        .strip_prefix("data:")
        .or_else(|| trimmed.strip_prefix("DATA:"))?;
    let (mediatype, payload) = rest.split_once(";base64,")?;
    if mediatype.is_empty() {
        return None;
    }
    base64::engine::general_purpose::STANDARD
        .decode(payload)
        .or_else(|_| base64::engine::general_purpose::STANDARD_NO_PAD.decode(payload))
        .ok()
}

pub async fn fetch(client: &HttpClient, req: RequestSpec) -> anyhow::Result<FetchResponse> {
    let req = {
        let mut req = req;
        req.url = resolve_proxy_url(&req.url);
        req
    };
    if let Some(bytes) = decode_data_uri(&req.url) {
        // Legado returns hex-encoded bytes when the url options declare a `type`
        // (sources then call java.hexDecodeToString), plain text otherwise.
        let body = if req
            .response_type
            .as_deref()
            .map(|value| !value.trim().is_empty())
            .unwrap_or(false)
        {
            hex::encode(&bytes)
        } else {
            String::from_utf8_lossy(&bytes).into_owned()
        };
        return Ok(FetchResponse {
            url: req.url,
            status: 200,
            body,
            content_type: None,
            headers: Vec::new(),
            is_successful: true,
        });
    }
    let mut last_err: Option<anyhow::Error> = None;
    let fast_fail = is_source_fast_fail_url(&req.url);
    if fast_fail {
        wait_for_source_fast_fail_host(&req.url).await;
    }
    let max_retries = if fast_fail { 0 } else { req.retry };
    for attempt in 0..=max_retries {
        let req = req.clone();
        let mut builder = match req.method {
            HttpMethod::GET => client.client().get(&req.url),
            HttpMethod::POST => client.client().post(&req.url),
        };
        if fast_fail {
            builder = builder.timeout(Duration::from_secs(SOURCE_FAST_FAIL_TIMEOUT_SECS));
        }

        let mut has_content_type = false;
        for (k, v) in &req.headers {
            if k.to_lowercase() == "content-type" {
                has_content_type = true;
            }
            builder = builder.header(k, v);
        }

        if let Some(body) = req.body {
            if matches!(req.method, HttpMethod::POST) && !has_content_type {
                builder = builder.header(
                    reqwest::header::CONTENT_TYPE,
                    "application/x-www-form-urlencoded",
                );
            }
            tracing::debug!(body_len = body.len(), "fetch sending request body");
            builder = builder.body(body);
        }

        let method = match req.method {
            HttpMethod::GET => "GET",
            HttpMethod::POST => "POST",
        };
        tracing::debug!(method, url = %req.url, "fetch executing request");
        match builder.send().await {
            Ok(res) => {
                let status = res.status().as_u16();
                tracing::debug!(status, "fetch response received");
                let is_successful = res.status().is_success();
                let url = res.url().to_string();
                let content_type = res
                    .headers()
                    .get(reqwest::header::CONTENT_TYPE)
                    .and_then(|v| v.to_str().ok())
                    .map(|s| s.to_string());
                let headers = res
                    .headers()
                    .iter()
                    .filter_map(|(name, value)| {
                        value
                            .to_str()
                            .ok()
                            .map(|value| (name.to_string(), value.to_string()))
                    })
                    .collect::<Vec<_>>();
                let bytes = res.bytes().await?;
                let mut body = if req
                    .response_type
                    .as_deref()
                    .map(|value| !value.trim().is_empty())
                    .unwrap_or(false)
                {
                    hex::encode(&bytes)
                } else {
                    decode_body(&bytes, req.charset.as_deref(), content_type.as_deref())
                };
                if is_xml_response(content_type.as_deref(), &body)
                    && !body.trim_start().starts_with("<?xml")
                {
                    body = format!("<?xml version=\"1.0\"?>{}", body);
                }
                if status >= 500 && attempt < max_retries {
                    last_err = Some(anyhow::anyhow!("server error status {}", status));
                } else {
                    return Ok(FetchResponse {
                        url,
                        status,
                        body,
                        content_type,
                        headers,
                        is_successful,
                    });
                }
            }
            Err(e) => {
                last_err = Some(e.into());
            }
        }

        if attempt < max_retries {
            let backoff = 200u64 * (attempt as u64 + 1);
            sleep(Duration::from_millis(backoff)).await;
        }
    }
    Err(last_err.unwrap_or_else(|| anyhow::anyhow!("fetch failed")))
}

pub(crate) fn is_source_fast_fail_url(url: &str) -> bool {
    source_fast_fail_host(url).is_some()
}

fn source_fast_fail_host(url: &str) -> Option<String> {
    let Ok(parsed) = reqwest::Url::parse(url.trim()) else {
        return None;
    };
    let Some(host) = parsed.host_str().map(|value| value.to_ascii_lowercase()) else {
        return None;
    };
    (host == "52dns.cc" || host.ends_with(".52dns.cc")).then_some(host)
}

async fn wait_for_source_fast_fail_host(url: &str) {
    let Some(host) = source_fast_fail_host(url) else {
        return;
    };
    let min_interval = Duration::from_millis(SOURCE_FAST_FAIL_MIN_INTERVAL_MS);
    let states = SOURCE_FAST_FAIL_HOST_STATES.get_or_init(|| Mutex::new(HashMap::new()));
    loop {
        let wait = {
            let mut guard = states.lock().await;
            let now = Instant::now();
            match guard.get(&host).copied() {
                Some(last_started) => {
                    let elapsed = now.saturating_duration_since(last_started);
                    if elapsed < min_interval {
                        min_interval - elapsed
                    } else {
                        guard.insert(host.clone(), now);
                        return;
                    }
                }
                None => {
                    guard.insert(host.clone(), now);
                    return;
                }
            }
        };
        sleep(wait).await;
    }
}

fn decode_body(bytes: &[u8], charset: Option<&str>, content_type: Option<&str>) -> String {
    let label = charset
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| charset_from_content_type(content_type))
        .or_else(|| charset_from_html_meta(bytes));

    if let Some(label) = label {
        if let Some(encoding) = Encoding::for_label(label.trim().as_bytes()) {
            let (text, _, _) = encoding.decode(bytes);
            return text.into_owned();
        }
    }

    String::from_utf8_lossy(bytes).into_owned()
}

fn charset_from_content_type(content_type: Option<&str>) -> Option<String> {
    content_type.and_then(|content_type| {
        content_type.split(';').find_map(|part| {
            let (key, value) = part.split_once('=')?;
            if key.trim().eq_ignore_ascii_case("charset") {
                let value = value.trim().trim_matches('"').trim_matches('\'');
                (!value.is_empty()).then(|| value.to_string())
            } else {
                None
            }
        })
    })
}

fn charset_from_html_meta(bytes: &[u8]) -> Option<String> {
    let sniff_len = bytes.len().min(4096);
    let head = String::from_utf8_lossy(&bytes[..sniff_len]);
    let lower = head.to_ascii_lowercase();
    let index = lower.find("charset")?;
    let after = &head[index + "charset".len()..];
    let after = after.trim_start();
    let after = after.strip_prefix('=').unwrap_or(after).trim_start();
    let after = after
        .strip_prefix('"')
        .or_else(|| after.strip_prefix('\''))
        .unwrap_or(after);
    let label = after
        .chars()
        .take_while(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_'))
        .collect::<String>();
    (!label.is_empty()).then_some(label)
}

impl From<FetchResponse> for StrResponse {
    fn from(value: FetchResponse) -> Self {
        Self {
            body: value.body,
            url: value.url,
            code: value.status,
            headers: value.headers,
            is_successful: value.is_successful,
        }
    }
}

impl From<StrResponse> for FetchResponse {
    fn from(value: StrResponse) -> Self {
        Self {
            url: value.url,
            status: value.code,
            body: value.body,
            content_type: None,
            headers: value.headers,
            is_successful: value.is_successful,
        }
    }
}

fn is_xml_response(content_type: Option<&str>, body: &str) -> bool {
    content_type
        .map(|value| value.to_ascii_lowercase().contains("xml"))
        .unwrap_or(false)
        || body.trim_start().starts_with("<rss")
        || body.trim_start().starts_with("<feed")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crawler::http_client::HttpClient;

    #[test]
    fn decode_data_uri_decodes_base64_payload_with_mediatype() {
        // base64("7276384138653862966")
        let url = "data:book_id;base64,NzI3NjM4NDEzODY1Mzg2Mjk2Ng==";
        assert_eq!(
            decode_data_uri(url).as_deref(),
            Some("7276384138653862966".as_bytes())
        );
    }

    #[test]
    fn decode_data_uri_ignores_proxy_convention_and_plain_urls() {
        // empty mediatype = project proxy-URL convention, handled elsewhere
        assert_eq!(decode_data_uri("data:;base64,aHR0cHM6Ly9hLmI="), None);
        assert_eq!(decode_data_uri("https://example.com"), None);
        assert_eq!(decode_data_uri("data:text/plain,hello"), None);
    }

    #[tokio::test]
    async fn fetch_serves_data_uri_without_network() {
        let client = HttpClient::new(5, None).unwrap();

        // with a response type the body is hex-encoded (legado behaviour)
        let res = fetch(
            &client,
            RequestSpec {
                url: "data:book_id;base64,NzI3NjM4NDEzODY1Mzg2Mjk2Ng==".to_string(),
                response_type: Some("M_xh".to_string()),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        assert_eq!(res.status, 200);
        assert!(res.is_successful);
        assert_eq!(res.body, hex::encode("7276384138653862966"));

        // without a response type the body is the decoded text
        let res = fetch(
            &client,
            RequestSpec {
                url: "data:book_id;base64,NzI3NjM4NDEzODY1Mzg2Mjk2Ng==".to_string(),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        assert_eq!(res.body, "7276384138653862966");
    }

    #[test]
    fn decode_body_uses_response_charset() {
        let bytes = b"\xd0\xa1\xcb\xb5\xca\xd5\xb2\xd8\xc5\xc5\xd0\xd0\xb0\xf1";

        let text = decode_body(bytes, None, Some("text/html; charset=gb2312"));

        assert_eq!(text, "小说收藏排行榜");
    }

    #[test]
    fn decode_body_detects_html_meta_charset() {
        let bytes = b"<meta http-equiv=\"content-type\" content=\"text/html;charset=gb2312\"><title>\xb7\xc9\xc2\xac\xd0\xa1\xcb\xb5</title>";

        let text = decode_body(bytes, None, None);

        assert!(text.contains("飞卢小说"));
    }

    #[test]
    fn source_fast_fail_detects_52dns_hosts_only() {
        assert!(is_source_fast_fail_url(
            "https://gofq.52dns.cc/content?item_id=1"
        ));
        assert!(is_source_fast_fail_url(
            "https://jh.52dns.cc/qimao/content.php?chapterId=1"
        ));
        assert!(!is_source_fast_fail_url(
            "https://reading.snssdk.com/reading/bookapi/detail/v/"
        ));
        assert!(!is_source_fast_fail_url("data:item_id;base64,MTIz"));
    }
}
