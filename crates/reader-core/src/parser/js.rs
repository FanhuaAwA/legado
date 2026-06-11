use crate::util::hash::md5_hex;
use crate::util::text::{apply_regex_replace, strip_whitespace};
use aes::Aes128;
use base64::Engine;
use cbc::cipher::{block_padding::Pkcs7, BlockDecryptMut, KeyIvInit};
use chrono::{Local, TimeZone};
use once_cell::sync::Lazy;
use reqwest::blocking::Client;
use reqwest::Method;
use rquickjs::function::Func;
use rquickjs::function::Opt;
use rquickjs::promise::MaybePromise;
use rquickjs::{Context, Object, Runtime, Value};
use serde_json::Value as JsonValue;
use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::sync::Mutex;
use uuid::Uuid;

/// Thread-local pool of QuickJS runtimes to avoid per-eval construction cost.
/// Runtimes are reused across rule evaluations; a fresh Context is created each
/// time to guarantee clean globals and prevent cross-source state leakage.
const MAX_POOLED_RUNTIMES: usize = 4;

thread_local! {
    static RUNTIME_POOL: RefCell<Vec<Runtime>> = const { RefCell::new(Vec::new()) };
}

fn acquire_runtime() -> Runtime {
    RUNTIME_POOL
        .with(|pool| pool.borrow_mut().pop())
        .unwrap_or_else(|| Runtime::new().expect("failed to create QuickJS runtime"))
}

fn release_runtime(rt: Runtime) {
    RUNTIME_POOL.with(|pool| {
        let mut pool = pool.borrow_mut();
        if pool.len() < MAX_POOLED_RUNTIMES {
            pool.push(rt);
        }
    });
}

static JS_KV: Lazy<Mutex<HashMap<String, String>>> = Lazy::new(|| Mutex::new(HashMap::new()));
static JS_LIB_CACHE: Lazy<Mutex<HashMap<String, String>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));
static JS_HTTP_CLIENT: Lazy<Client> = Lazy::new(|| {
    // reqwest::blocking 客户端不能在 tokio 异步上下文中构建；Lazy 首次触发点
    // 可能位于异步规则引擎线程，因此固定在独立线程上完成构建。
    std::thread::spawn(|| {
        Client::builder()
            .cookie_store(true)
            .gzip(true)
            .brotli(true)
            .deflate(true)
            .build()
            .expect("failed to build JS HTTP client")
    })
    .join()
    .expect("JS HTTP client init thread panicked")
});

/// Dedicated thread pool for reqwest::blocking HTTP calls (used by java.ajax / legado.http).
///
/// Per-call `std::thread::scope::spawn` creates one OS thread per request and leaks
/// a temporary tokio runtime per thread (reqwest::blocking behavior). Under concurrent
/// prefetch or multi-source search, this causes tokio worker starvation and excessive
/// thread churn. This pool caps worker count and reuses them across calls.
const HTTP_POOL_SIZE: usize = 4;

type HttpWork = (
    reqwest::blocking::RequestBuilder,
    std::sync::mpsc::Sender<anyhow::Result<String>>,
);

static HTTP_WORK_TX: Lazy<std::sync::mpsc::Sender<HttpWork>> = Lazy::new(|| {
    let (tx, rx) = std::sync::mpsc::channel::<HttpWork>();
    let rx = std::sync::Arc::new(std::sync::Mutex::new(rx));
    for i in 0..HTTP_POOL_SIZE {
        let rx = std::sync::Arc::clone(&rx);
        std::thread::Builder::new()
            .name(format!("js_http_pool_{i}"))
            .spawn(move || {
                while let Ok((req, reply)) = rx.lock().unwrap_or_else(|e| e.into_inner()).recv() {
                    let result = (|| -> anyhow::Result<String> {
                        let response = req.send()?;
                        Ok(response.text().unwrap_or_default())
                    })();
                    let _ = reply.send(result);
                }
            })
            .expect("failed to spawn js HTTP pool thread");
    }
    tx
});

/// Execute a blocking HTTP request on the dedicated worker pool.
///
/// Sends the request to the pool via channel and blocks the calling thread on a
/// oneshot reply. This MUST NOT be called from inside a tokio worker thread
/// (see `send_text_blocking` which wraps this with `std::thread::scope`).
fn send_text_blocking_pool(target: reqwest::blocking::RequestBuilder) -> anyhow::Result<String> {
    let (reply_tx, reply_rx) = std::sync::mpsc::channel();
    HTTP_WORK_TX
        .send((target, reply_tx))
        .map_err(|_| anyhow::anyhow!("js HTTP pool shut down"))?;
    reply_rx
        .recv()
        .map_err(|_| anyhow::anyhow!("js HTTP worker panicked"))?
}

/// Execute a reqwest::blocking request off the tokio runtime.
///
/// reqwest::blocking internally creates and drops a temporary tokio runtime; calling
/// it directly from an async context triggers "Cannot drop a runtime in a context
/// where blocking is not allowed". This function bridges the call to a plain OS thread
/// (via the dedicated pool) so the calling tokio worker is not polluted.
fn send_text_blocking(req: reqwest::blocking::RequestBuilder) -> anyhow::Result<String> {
    send_text_blocking_pool(req)
}
static JS_DEVICE_ID: Lazy<String> = Lazy::new(|| {
    let mut map = JS_KV.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(existing) = map.get("__device_id") {
        return existing.clone();
    }
    let generated = Uuid::new_v4().to_string();
    map.insert("__device_id".to_string(), generated.clone());
    generated
});
type Aes128CbcDecryptor = cbc::Decryptor<Aes128>;
#[derive(Debug, Clone, Default)]
struct ActiveJsContext {
    js_lib: Option<String>,
    login_url: Option<String>,
    book_source_name: Option<String>,
    book_source_url: Option<String>,
    toc_url: Option<String>,
    chapter_url: Option<String>,
    chapter_title: Option<String>,
    chapter_index: Option<i32>,
}

thread_local! {
    static ACTIVE_JS_CONTEXT: RefCell<ActiveJsContext> = const { RefCell::new(ActiveJsContext {
        js_lib: None,
        login_url: None,
        book_source_name: None,
        book_source_url: None,
        toc_url: None,
        chapter_url: None,
        chapter_title: None,
        chapter_index: None,
    }) };
}

#[derive(Debug, Clone)]
pub enum JsSourceArg {
    String(String),
    Int(i32),
    Float(f64),
    Bool(bool),
    Json(JsonValue),
    Null,
}

pub fn with_js_lib<T>(js_lib: Option<&str>, f: impl FnOnce() -> T) -> T {
    with_js_source(js_lib, None, None, None, None, None, None, None, f)
}

pub fn with_js_source<T>(
    js_lib: Option<&str>,
    login_url: Option<&str>,
    book_source_name: Option<&str>,
    book_source_url: Option<&str>,
    toc_url: Option<&str>,
    chapter_url: Option<&str>,
    chapter_title: Option<&str>,
    chapter_index: Option<i32>,
    f: impl FnOnce() -> T,
) -> T {
    ACTIVE_JS_CONTEXT.with(|cell| {
        let previous = cell.replace(ActiveJsContext {
            js_lib: js_lib.map(|value| value.to_string()),
            login_url: login_url.map(|value| value.to_string()),
            book_source_name: book_source_name.map(|value| value.to_string()),
            book_source_url: book_source_url.map(|value| value.to_string()),
            toc_url: toc_url.map(|value| value.to_string()),
            chapter_url: chapter_url.map(|value| value.to_string()),
            chapter_title: chapter_title.map(|value| value.to_string()),
            chapter_index,
        });
        let result = f();
        cell.replace(previous);
        result
    })
}

/// Set book/chapter context for JS rule evaluation during content/toc paths.
/// This populates the `book` and `chapter` JS globals so sources using
/// `book.bookUrl`, `chapter.url`, etc. work without modifying source JSON.
pub fn with_book_context<T>(
    book_url: Option<&str>,
    toc_url: Option<&str>,
    chapter_url: Option<&str>,
    chapter_title: Option<&str>,
    chapter_index: Option<i32>,
    f: impl FnOnce() -> T,
) -> T {
    ACTIVE_JS_CONTEXT.with(|cell| {
        let mut ctx = cell.borrow().clone();
        if let Some(v) = book_url {
            ctx.book_source_url = Some(v.to_string());
        }
        if let Some(v) = toc_url {
            ctx.toc_url = Some(v.to_string());
        }
        if let Some(v) = chapter_url {
            ctx.chapter_url = Some(v.to_string());
        }
        if let Some(v) = chapter_title {
            ctx.chapter_title = Some(v.to_string());
        }
        if let Some(v) = chapter_index {
            ctx.chapter_index = Some(v);
        }
        let previous = cell.replace(ctx);
        let result = f();
        cell.replace(previous);
        result
    })
}

pub fn eval_js(script: &str, input: &str, base_url: &str) -> anyhow::Result<String> {
    eval_js_inner(script, Some(input), Some(base_url), None, None, None)
}

pub fn eval_js_with_bindings(
    script: &str,
    input: &str,
    base_url: &str,
    bindings: &HashMap<String, JsonValue>,
) -> anyhow::Result<String> {
    eval_js_inner(
        script,
        Some(input),
        Some(base_url),
        None,
        None,
        Some(bindings),
    )
}

pub fn eval_js_search_with_source(
    script: &str,
    key: &str,
    page: i32,
    source_key: &str,
) -> anyhow::Result<String> {
    eval_js_inner_with_source(
        script,
        None,
        None,
        Some(key),
        Some(page),
        Some(source_key),
        None,
    )
}

pub fn eval_js_url(
    script: &str,
    result: &str,
    key: &str,
    page: i32,
    source_key: &str,
    base_url: &str,
) -> anyhow::Result<String> {
    eval_js_inner_with_source(
        script,
        Some(result),
        Some(base_url),
        Some(key),
        Some(page),
        Some(source_key),
        None,
    )
}

pub fn eval_source_function(
    source: &str,
    function_name: &str,
    args: &[JsSourceArg],
) -> anyhow::Result<String> {
    if !is_safe_js_identifier(function_name) {
        anyhow::bail!("invalid JS function name: {function_name}");
    }
    let args = args
        .iter()
        .map(js_source_arg_literal)
        .collect::<anyhow::Result<Vec<_>>>()?
        .join(",");
    let script = format!(
        r#"
{source}
;(async () => {{
  const __fn = {function_name};
  if (typeof __fn !== 'function') {{
    throw new Error('missing source function: {function_name}');
  }}
  const __value = await __fn({args});
  if (__value === undefined || __value === null) {{
    return '';
  }}
  if (typeof __value === 'string') {{
    return __value;
  }}
  return JSON.stringify(__value);
}})()
"#
    );
    eval_js(&script, "", "")
}

pub fn eval_source_function_value(
    source: &str,
    function_name: &str,
    args: &[JsSourceArg],
) -> anyhow::Result<JsonValue> {
    let raw = eval_source_function(source, function_name, args)?;
    Ok(serde_json::from_str::<JsonValue>(&raw).unwrap_or(JsonValue::String(raw)))
}

fn eval_js_inner(
    script: &str,
    input: Option<&str>,
    base_url: Option<&str>,
    key: Option<&str>,
    page: Option<i32>,
    bindings: Option<&HashMap<String, JsonValue>>,
) -> anyhow::Result<String> {
    eval_js_inner_with_source(script, input, base_url, key, page, None, bindings)
}

fn eval_js_inner_with_source(
    script: &str,
    input: Option<&str>,
    base_url: Option<&str>,
    key: Option<&str>,
    page: Option<i32>,
    source_key: Option<&str>,
    bindings: Option<&HashMap<String, JsonValue>>,
) -> anyhow::Result<String> {
    let rt = acquire_runtime();
    let ctx = Context::full(&rt)?;
    let result = ctx.with(|ctx| {
        let globals = ctx.globals();
        let input_value = input.unwrap_or("");
        let base_url_value = base_url.unwrap_or("");
        let active_context = active_js_context();
        let shared_js = active_js_lib_script(&active_context)?;

        globals.set("input", input_value)?;
        globals.set("result", input_value)?;
        globals.set("src", input_value)?;
        globals.set("base_url", base_url_value)?;
        globals.set("baseUrl", base_url_value)?;
        if let Some(key) = key {
            globals.set("key", key)?;
        }
        if let Some(page) = page {
            globals.set("page", page)?;
        }

        // Default url variable for Legado compatibility
        globals.set("url", base_url_value)?;

        // Legado source object — provides source-scoped variables, login info, and key
        let source_key_val = source_key.unwrap_or("").to_string();
        let source_obj = Object::new(ctx.clone())?;
        let source_name_value = active_context.book_source_name.clone().unwrap_or_default();
        let login_url_value = active_context
            .login_url
            .clone()
            .map(|value| prepare_legacy_js_script(&value))
            .unwrap_or_default();
        let sk_clone = source_key_val.clone();
        source_obj.set("key", source_key_val.clone())?;
        source_obj.set("bookSourceName", source_name_value.clone())?;
        source_obj.set("loginUrl", login_url_value.clone())?;
        source_obj.set("getKey", Func::new(move || sk_clone.clone()))?;

        let sk_for_login_any = source_key_val.clone();
        source_obj.set(
            "getLoginInfo",
            Func::new(move || -> bool {
                let map = JS_KV.lock().unwrap_or_else(|e| e.into_inner());
                let prefix = source_login_prefix(&sk_for_login_any);
                map.keys().any(|key| key.starts_with(&prefix))
            }),
        )?;

        let sk_for_login_json = source_key_val.clone();
        source_obj.set(
            "__getLoginInfoJson",
            Func::new(move || -> String {
                let map = JS_KV.lock().unwrap_or_else(|e| e.into_inner());
                let prefix = source_login_prefix(&sk_for_login_json);
                let items = map
                    .iter()
                    .filter(|(k, _)| k.starts_with(&prefix))
                    .map(|(k, v)| (k.replacen(&prefix, "", 1), JsonValue::String(v.clone())))
                    .collect::<BTreeMap<_, _>>();
                serde_json::to_string(&items).unwrap_or_else(|_| "{}".to_string())
            }),
        )?;

        let sk_for_login_get = source_key_val.clone();
        source_obj.set(
            "__getLoginInfoValue",
            Func::new(move |key: String| -> String {
                let map = JS_KV.lock().unwrap_or_else(|e| e.into_inner());
                map.get(&source_login_key(&sk_for_login_get, &key))
                    .cloned()
                    .unwrap_or_default()
            }),
        )?;

        let sk_for_login_set = source_key_val.clone();
        source_obj.set(
            "__setLoginInfoValue",
            Func::new(move |key: String, value: Value<'_>| -> bool {
                let mut map = JS_KV.lock().unwrap_or_else(|e| e.into_inner());
                map.insert(
                    source_login_key(&sk_for_login_set, &key),
                    js_callback_arg_to_string(value),
                );
                true
            }),
        )?;

        let sk_for_login_clear = source_key_val.clone();
        source_obj.set(
            "__clearLoginInfo",
            Func::new(move || -> bool {
                let mut map = JS_KV.lock().unwrap_or_else(|e| e.into_inner());
                let prefix = source_login_prefix(&sk_for_login_clear);
                let keys: Vec<String> = map
                    .keys()
                    .filter(|k| k.starts_with(&prefix))
                    .cloned()
                    .collect();
                for key in keys {
                    map.remove(&key);
                }
                tracing::debug!(target: "reader_core::js_source::login", "clearLoginInfo removed keys matching prefix={prefix}");
                true
            }),
        )?;

        let sk_for_var = source_key_val.clone();
        source_obj.set(
            "getVariable",
            Func::new(move |key: Opt<String>| -> String {
                let map = JS_KV.lock().unwrap_or_else(|e| e.into_inner());
                let full_key = source_variable_key(&sk_for_var, key.0.as_deref());
                map.get(&full_key).cloned().unwrap_or_default()
            }),
        )?;
        let sk_for_set = source_key_val.clone();
        source_obj.set(
            "setVariable",
            Func::new(move |key_or_value: String, value: Opt<String>| -> bool {
                let mut map = JS_KV.lock().unwrap_or_else(|e| e.into_inner());
                let (name, val) = match value.0 {
                    Some(val) => (Some(key_or_value.as_str()), val),
                    None => (None, key_or_value),
                };
                let full_key = source_variable_key(&sk_for_set, name);
                map.insert(full_key, val);
                true
            }),
        )?;
        let sk_for_put = source_key_val.clone();
        source_obj.set(
            "putVariable",
            Func::new(move |key_or_value: String, value: Opt<String>| -> bool {
                let mut map = JS_KV.lock().unwrap_or_else(|e| e.into_inner());
                let (name, val) = match value.0 {
                    Some(val) => (Some(key_or_value.as_str()), val),
                    None => (None, key_or_value),
                };
                let full_key = source_variable_key(&sk_for_put, name);
                map.insert(full_key, val);
                true
            }),
        )?;
        globals.set("source", source_obj)?;

        let cookie_obj = Object::new(ctx.clone())?;
        cookie_obj.set(
            "getCookie",
            Func::new(|domain: String, name: Opt<String>| -> String {
                get_cookie_value(&domain, name.0.as_deref())
            }),
        )?;
        cookie_obj.set(
            "setCookie",
            Func::new(|domain: String, name_or_value: String, value: Opt<String>| -> bool {
                let mut map = JS_KV.lock().unwrap_or_else(|e| e.into_inner());
                let (name, val) = match value.0 {
                    Some(val) => (Some(name_or_value.as_str()), val),
                    None => (None, name_or_value),
                };
                map.insert(cookie_key(&domain, name), val);
                true
            }),
        )?;
        cookie_obj.set(
            "removeCookie",
            Func::new(|domain: String, name: Opt<String>| -> String {
                let mut map = JS_KV.lock().unwrap_or_else(|e| e.into_inner());
                map.remove(&cookie_key(&domain, name.0.as_deref()));
                if name.0.is_none() {
                    let prefix = format!("__cookie_{}::", domain);
                    map.retain(|key, _| !key.starts_with(&prefix));
                }
                "".to_string()
            }),
        )?;
        cookie_obj.set(
            "getKey",
            Func::new(|domain: String, name: Opt<String>| -> String {
                get_cookie_value(&domain, name.0.as_deref())
            }),
        )?;
        globals.set("cookie", cookie_obj)?;

        let cache_obj = Object::new(ctx.clone())?;
        cache_obj.set(
            "get",
            Func::new(|key: String| -> Option<String> {
                let map = JS_KV.lock().unwrap_or_else(|e| e.into_inner());
                map.get(&key).cloned()
            }),
        )?;
        cache_obj.set(
            "put",
            Func::new(|key: String, val: Value<'_>| -> bool {
                let mut map = JS_KV.lock().unwrap_or_else(|e| e.into_inner());
                map.insert(key, js_callback_arg_to_string(val));
                true
            }),
        )?;
        cache_obj.set(
            "getMemory",
            Func::new(|key: String| -> Option<String> {
                let map = JS_KV.lock().unwrap_or_else(|e| e.into_inner());
                map.get(&key).cloned()
            }),
        )?;
        cache_obj.set(
            "putMemory",
            Func::new(|key: String, val: Value<'_>| -> bool {
                let mut map = JS_KV.lock().unwrap_or_else(|e| e.into_inner());
                map.insert(key, js_callback_arg_to_string(val));
                true
            }),
        )?;
        cache_obj.set(
            "delete",
            Func::new(|key: String| -> String {
                let mut map = JS_KV.lock().unwrap_or_else(|e| e.into_inner());
                map.remove(&key);
                "".to_string()
            }),
        )?;
        globals.set("cache", cache_obj)?;

        let java_obj = Object::new(ctx.clone())?;
        java_obj.set(
            "ajax",
            Func::new(|spec: String| -> String { java_ajax(&spec).unwrap_or_default() }),
        )?;
        java_obj.set(
            "md5Encode",
            Func::new(|input: String| -> String { md5_hex(&input) }),
        )?;
        java_obj.set(
            "md5Encode16",
            Func::new(|input: String| -> String {
                let full = md5_hex(&input);
                full.chars().take(16).collect()
            }),
        )?;
        java_obj.set(
            "timeFormat",
            Func::new(|timestamp: i64| -> String { java_time_format(timestamp) }),
        )?;
        java_obj.set(
            "timeFormatUTC",
            Func::new(
                |timestamp: i64, format: Opt<String>, offset_hours: Opt<i32>| -> String {
                    let fmt = format.0.unwrap_or_else(|| "%Y-%m-%dT%H:%M:%S".to_string());
                    let offset = offset_hours.0.unwrap_or(0);
                    let chrono_fmt = java_format_to_chrono(&fmt);
                    let base = chrono::Utc
                        .timestamp_millis_opt(timestamp)
                        .single()
                        .or_else(|| {
                            chrono::Utc
                                .timestamp_opt(timestamp / 1000, (timestamp % 1000 * 1_000_000) as u32)
                                .single()
                        });
                    base.map(|dt| {
                        let adjusted = dt + chrono::Duration::hours(offset as i64);
                        adjusted.format(&chrono_fmt).to_string()
                    })
                    .unwrap_or_default()
                },
            ),
        )?;


        java_obj.set(
            "getVerificationCode",
            Func::new(|image_url: String| -> String {
                tracing::warn!(
                    target: "reader_core::js_source",
                    "getVerificationCode requires interactive verification and is unavailable in headless JS runtime: {image_url}"
                );
                String::new()
            }),
        )?;
        java_obj.set(
            "base64DecodeToByteArray",
            Func::new(|input: String| -> String {
                decode_base64_to_utf8(&input).unwrap_or_default()
            }),
        )?;
        java_obj.set(
            "__base64DecodeToByteArrayBase64",
            Func::new(|input: String| -> String {
                decode_base64_bytes(&input)
                    .map(|bytes| base64::engine::general_purpose::STANDARD.encode(bytes))
                    .unwrap_or_default()
            }),
        )?;
        java_obj.set(
            "__base64DecodeToUtf8",
            Func::new(|input: String| -> String {
                decode_base64_to_utf8(&input).unwrap_or_default()
            }),
        )?;
        java_obj.set(
            "toast",
            Func::new(|msg: Opt<Value<'_>>| -> bool {
                let msg = msg.0.map(js_callback_arg_to_string).unwrap_or_default();
                tracing::info!(target: "reader_core::js_source::toast", "{msg}");
                true
            }),
        )?;
        java_obj.set(
            "longToast",
            Func::new(|msg: Opt<Value<'_>>| -> bool {
                let msg = msg.0.map(js_callback_arg_to_string).unwrap_or_default();
                tracing::info!(target: "reader_core::js_source::toast", "long: {msg}");
                true
            }),
        )?;
        java_obj.set(
            "log",
            Func::new(|msg: Opt<Value<'_>>| -> bool {
                let msg = msg.0.map(js_callback_arg_to_string).unwrap_or_default();
                tracing::info!(target: "reader_core::js_source::log", "{msg}");
                true
            }),
        )?;
        let input_for_get_string = input_value.to_string();
        java_obj.set(
            "getString",
            Func::new(move |path: String| -> String { java_get_string(&input_for_get_string, &path) }),
        )?;
        java_obj.set(
            "getReadBookConfigMap",
            Func::new(|| -> String { "{}".to_string() }),
        )?;
        java_obj.set(
            "getThemeConfigMap",
            Func::new(|| -> String { "{}".to_string() }),
        )?;
        java_obj.set("getThemeMode", Func::new(|| -> i32 { 0 }))?;
        java_obj.set(
            "androidId",
            Func::new(|| -> String { JS_DEVICE_ID.clone() }),
        )?;
        java_obj.set("deviceID", Func::new(|| -> String { JS_DEVICE_ID.clone() }))?;
        java_obj.set(
            "get",
            Func::new(|url: String| -> String {
                if is_http_url(&url) {
                    java_request_simple("GET", &url, None).unwrap_or_default()
                } else {
                    let map = JS_KV.lock().unwrap_or_else(|e| e.into_inner());
                    map.get(&java_storage_key(&url)).cloned().unwrap_or_default()
                }
            }),
        )?;
        java_obj.set(
            "post",
            Func::new(|url: String, body: String| -> String {
                java_request_simple("POST", &url, Some(body)).unwrap_or_default()
            }),
        )?;
        java_obj.set(
            "put",
            Func::new(|key: String, value: Value<'_>| -> String {
                let text = js_callback_arg_to_string(value);
                if is_http_url(&key) {
                    java_request_simple("PUT", &key, Some(text)).unwrap_or_default()
                } else {
                    let mut map = JS_KV.lock().unwrap_or_else(|e| e.into_inner());
                    map.insert(java_storage_key(&key), text.clone());
                    text
                }
            }),
        )?;
        java_obj.set(
            "base64Encode",
            Func::new(|input: String| -> String {
                base64::engine::general_purpose::STANDARD.encode(input)
            }),
        )?;
        java_obj.set(
            "base64Decode",
            Func::new(|input: String| -> String {
                base64::engine::general_purpose::STANDARD
                    .decode(input)
                    .ok()
                    .and_then(|bytes| String::from_utf8(bytes).ok())
                    .unwrap_or_default()
            }),
        )?;
        java_obj.set(
            "hexDecodeToString",
            Func::new(|hex: String| -> String {
                decode_hex_to_utf8_string(&hex)
            }),
        )?;
        java_obj.set(
            "__ajaxAll",
            Func::new(|specs_json: String| -> String {
                let specs = serde_json::from_str::<Vec<String>>(&specs_json).unwrap_or_default();
                let mut results: Vec<String> = Vec::new();
                for spec in specs {
                    let result = java_ajax(&spec).unwrap_or_default();
                    results.push(result);
                }
                serde_json::to_string(&results).unwrap_or_else(|_| "[]".to_string())
            }),
        )?;
        java_obj.set(
            "startBrowser",
            Func::new(|url: String| -> String {
                tracing::warn!(target: "reader_core::js_source", "startBrowser called but not supported on this platform: {url}");
                "".to_string()
            }),
        )?;
        java_obj.set(
            "startBrowserAwait",
            Func::new(|url: String, title: Opt<String>| -> String {
                tracing::warn!(
                    target: "reader_core::js_source",
                    "startBrowserAwait called but not supported on this platform: url={url}, title={:?}",
                    title.0
                );
                "".to_string()
            }),
        )?;
        java_obj.set("showBrowser", Func::new(|_url: String, _title: Opt<String>| -> bool { false }))?;
        java_obj.set("open", Func::new(|_kind: String, _target: String, _extra: Opt<String>| -> bool { false }))?;
        java_obj.set("refreshExplore", Func::new(|| -> bool { false }))?;
        java_obj.set("searchBook", Func::new(|_keyword: String, _source: Opt<String>| -> bool { false }))?;
        java_obj.set("reLoginView", Func::new(|| -> bool { false }))?;
        java_obj.set(
            "upConfig",
            Func::new(|value: Value<'_>| -> bool {
                let text = js_callback_arg_to_string(value);
                let len = text.len();
                let mut map = JS_KV.lock().unwrap_or_else(|e| e.into_inner());
                map.insert("__java_upConfig".to_string(), text);
                tracing::debug!(target: "reader_core::js_source::config", "upConfig stored {len} bytes");
                true
            }),
        )?;
        java_obj.set(
            "upLoginData",
            Func::new(|value: Value<'_>| -> bool {
                let text = js_callback_arg_to_string(value);
                let len = text.len();
                let mut map = JS_KV.lock().unwrap_or_else(|e| e.into_inner());
                map.insert("__java_upLoginData".to_string(), text);
                tracing::debug!(target: "reader_core::js_source::login", "upLoginData stored {len} bytes");
                true
            }),
        )?;
        java_obj.set(
            "connect",
            Func::new(|spec: Opt<String>| -> String {
                match spec.0 {
                    Some(s) => java_request_simple("CONNECT", &s, None).unwrap_or_default(),
                    None => "".to_string(),
                }
            }),
        )?;
        java_obj.set(
            "getCookie",
            Func::new(|domain: String, name: Opt<String>| -> String {
                get_cookie_value(&domain, name.0.as_deref())
            }),
        )?;
        java_obj.set(
            "removeCookie",
            Func::new(|domain: String, name: Opt<String>| -> String {
                let mut map = JS_KV.lock().unwrap_or_else(|e| e.into_inner());
                map.remove(&cookie_key(&domain, name.0.as_deref()));
                "".to_string()
            }),
        )?;
        java_obj.set(
            "aesBase64DecodeToString",
            Func::new(
                |input: String, key: String, algorithm: String, iv: String| -> String {
                    java_aes_base64_decode_to_string(&input, &key, &algorithm, &iv)
                },
            ),
        )?;
        java_obj.set(
            "encodeURIComponent",
            Func::new(|input: String| -> String { urlencoding::encode(&input).into_owned() }),
        )?;
        java_obj.set(
            "decodeURIComponent",
            Func::new(|input: String| -> String {
                urlencoding::decode(&input)
                    .map(|s| s.into_owned())
                    .unwrap_or_default()
            }),
        )?;
        java_obj.set(
            "encodeURI",
            Func::new(|input: String| -> String { urlencoding::encode(&input).into_owned() }),
        )?;
        java_obj.set(
            "decodeURI",
            Func::new(|input: String| -> String {
                urlencoding::decode(&input)
                    .map(|s| s.into_owned())
                    .unwrap_or_default()
            }),
        )?;
        java_obj.set(
            "now",
            Func::new(|| -> i64 { chrono::Utc::now().timestamp_millis() }),
        )?;
        java_obj.set(
            "uuid",
            Func::new(|| -> String { Uuid::new_v4().to_string() }),
        )?;
        globals.set("java", java_obj)?;

        let digest_obj = Object::new(ctx.clone())?;
        digest_obj.set("md5Hex", Func::new(|input: String| -> String { md5_hex(&input) }))?;
        globals.set("DigestUtil", digest_obj)?;

        let str_obj = Object::new(ctx.clone())?;
        str_obj.set(
            "reverse",
            Func::new(|input: String| -> String { input.chars().rev().collect::<String>() }),
        )?;
        globals.set("StrUtil", str_obj)?;

        let zip_obj = Object::new(ctx.clone())?;
        zip_obj.set("gzip", Func::new(|input: String, _charset: Opt<String>| -> String { input }))?;
        zip_obj.set("unGzip", Func::new(|input: String, _charset: Opt<String>| -> String { input }))?;
        globals.set("ZipUtil", zip_obj)?;

        let base64_obj = Object::new(ctx.clone())?;
        base64_obj.set(
            "encode",
            Func::new(|input: String| -> String {
                base64::engine::general_purpose::STANDARD.encode(input)
            }),
        )?;
        base64_obj.set(
            "decode",
            Func::new(|input: String| -> String {
                base64::engine::general_purpose::STANDARD
                    .decode(input)
                    .ok()
                    .and_then(|bytes| String::from_utf8(bytes).ok())
                    .unwrap_or_default()
            }),
        )?;
        globals.set("Base64", base64_obj)?;

        let http_obj = Object::new(ctx.clone())?;
        http_obj.set(
            "get",
            Func::new(|url: String, headers: Opt<Value<'_>>| -> String {
                legado_http_request("GET", &url, None, headers.0).unwrap_or_default()
            }),
        )?;
        http_obj.set(
            "post",
            Func::new(
                |url: String, body: Opt<Value<'_>>, headers: Opt<Value<'_>>| -> String {
                    let body = body.0.map(js_callback_arg_to_string).unwrap_or_default();
                    legado_http_request("POST", &url, Some(body), headers.0).unwrap_or_default()
                },
            ),
        )?;
        http_obj.set(
            "request",
            Func::new(|options: Value<'_>| -> String {
                legado_http_request_options(&js_callback_arg_to_string(options)).unwrap_or_default()
            }),
        )?;
        http_obj.set(
            "fetch",
            Func::new(|url: String, headers: Opt<Value<'_>>| -> String {
                legado_http_request("GET", &url, None, headers.0).unwrap_or_default()
            }),
        )?;
        let legado_obj = Object::new(ctx.clone())?;
        legado_obj.set("http", http_obj)?;
        legado_obj.set(
            "log",
            Func::new(|message: Opt<Value<'_>>| -> bool {
                let message = message.0.map(js_callback_arg_to_string).unwrap_or_default();
                tracing::debug!(target: "reader_core::js_source", "{message}");
                true
            }),
        )?;
        globals.set("legado", legado_obj)?;

        globals.set(
            "kv_get",
            Func::new(|key: String| -> Option<String> {
                let map = JS_KV.lock().unwrap_or_else(|e| e.into_inner());
                map.get(&key).cloned()
            }),
        )?;
        globals.set(
            "kv_put",
            Func::new(|key: String, val: String| -> bool {
                let mut map = JS_KV.lock().unwrap_or_else(|e| e.into_inner());
                map.insert(key, val);
                true
            }),
        )?;
        globals.set(
            "regex_replace",
            Func::new(
                |input: String, pattern: String, replace: String| -> String {
                    apply_regex_replace(&input, &pattern, &replace)
                },
            ),
        )?;
        globals.set(
            "strip_ws",
            Func::new(|input: String| -> String { strip_whitespace(&input) }),
        )?;

        // Populate book object from active context (needed by rule content/toc JS paths)
        let book_obj = Object::new(ctx.clone())?;
        book_obj.set(
            "bookUrl",
            active_context.book_source_url.clone().unwrap_or_default(),
        )?;
        book_obj.set("name", active_context.book_source_name.clone().unwrap_or_default())?;
        book_obj.set(
            "tocUrl",
            active_context.toc_url.clone().unwrap_or_default(),
        )?;
        book_obj.set("author", "")?;
        // Legado compatibility: book.getVariable(key) reads from source-scoped variable store.
        // Used by 番茄 tocUrl JS: book.getVariable("custom")
        let sk_for_book_var = source_key_val.clone();
        book_obj.set(
            "getVariable",
            Func::new(move |key: Opt<String>| -> String {
                let map = JS_KV.lock().unwrap_or_else(|e| e.into_inner());
                let full_key = source_variable_key(&sk_for_book_var, key.0.as_deref());
                map.get(&full_key).cloned().unwrap_or_default()
            }),
        )?;
        globals.set("book", book_obj)?;

        // Populate chapter object from active context
        let chapter_obj = Object::new(ctx.clone())?;
        chapter_obj.set(
            "url",
            active_context
                .chapter_url
                .clone()
                .unwrap_or_else(|| base_url_value.to_string()),
        )?;
        chapter_obj.set(
            "title",
            active_context.chapter_title.clone().unwrap_or_default(),
        )?;
        chapter_obj.set(
            "index",
            active_context.chapter_index.unwrap_or(0),
        )?;
        chapter_obj.set("isVip", false)?;
        globals.set("chapter", chapter_obj)?;

        globals.set(
            "nextChapterUrl",
            active_context.chapter_url.clone().unwrap_or_default(),
        )?;
        globals.set("title", "")?;
        globals.set("rssArticle", Object::new(ctx.clone())?)?;

        install_legado_compat_prelude(ctx.clone())?;

        if let Some(bindings) = bindings {
            for (key, value) in bindings {
                let js_value = ctx.json_parse(value.to_string())?;
                globals.set(key.as_str(), js_value)?;
            }
        }

        let script = inline_source_login_url(script, active_context.login_url.as_deref());
        let script = if shared_js.trim().is_empty() {
            script
        } else {
            format!("{shared_js}\n;\n{script}")
        };
        let v = eval_script(ctx.clone(), &script)?;
        js_value_to_string(ctx, v)
    });
    release_runtime(rt);
    result
}

fn js_value_to_string<'js>(ctx: rquickjs::Ctx<'js>, value: Value<'js>) -> anyhow::Result<String> {
    let value = match MaybePromise::from_value(value).finish::<Value>() {
        Ok(value) => value,
        Err(err) => {
            if let Some(exception) = ctx.catch().into_exception() {
                return Err(anyhow::anyhow!("JS Exception: {:?}", exception));
            }
            return Err(err.into());
        }
    };

    let result = if value.is_null() || value.is_undefined() {
        String::new()
    } else if let Some(s) = value.clone().into_string() {
        let s: rquickjs::String<'_> = s;
        s.to_string()
            .map(|value| value.to_string())
            .unwrap_or_default()
    } else {
        match ctx.json_stringify(value) {
            Ok(Some(json)) => json.to_string().unwrap_or_default(),
            _ => String::new(),
        }
    };
    Ok(result)
}

fn active_js_context() -> ActiveJsContext {
    ACTIVE_JS_CONTEXT.with(|cell| cell.borrow().clone())
}

fn install_legado_compat_prelude(ctx: rquickjs::Ctx<'_>) -> anyhow::Result<()> {
    ctx.eval::<(), _>(
        r#"
// ── Packages stub (Android/Rhino compatibility) ──
if (typeof globalThis.Packages === 'undefined') {
  globalThis.Packages = {
    okhttp3: null,  // populated below
    cn: { hutool: { core: { util: null, codec: null }, crypto: { digest: null } } },
    android: {
      os: {
        Build: {
          BRAND: 'generic',
          MODEL: 'LegadoTauri',
          DISPLAY: 'LegadoTauri',
          VERSION: { SDK_INT: 35, RELEASE: '15' }
        }
      }
    }
  };
}

// ── okhttp3 shim — translates OkHttp calls to java.ajax ──
(function() {
  var _okhttp = {};
  var _sharedClient = null;

  _okhttp.MediaType = { parse: function(mt) { return { type: mt }; } };

  function _isMediaType(value) {
    return value && typeof value === 'object' && typeof value.type !== 'undefined';
  }

  function _byteArrayBase64(value) {
    if (value && typeof value === 'object' && typeof value.__legadoByteArrayBase64 === 'string') {
      return value.__legadoByteArrayBase64;
    }
    return null;
  }

  function _requestBody(content, mediaType) {
    var body = { mediaType: mediaType || null };
    var base64 = _byteArrayBase64(content);
    if (base64 !== null) {
      body.bodyBase64 = base64;
      return body;
    }
    body.content = content === undefined || content === null ? '' : String(content);
    return body;
  }

  _okhttp.RequestBody = {
    create: function(first, second) {
      if (_isMediaType(first)) {
        return _requestBody(second, first);
      }
      return _requestBody(first, _isMediaType(second) ? second : null);
    }
  };

  _okhttp.FormBody = { Builder: function() {
    this._pairs = [];
    this.add = function(name, value) {
      this._pairs.push(encodeURIComponent(name) + '=' + encodeURIComponent(value));
      return this;
    };
    this.addEncoded = function(name, value) {
      this._pairs.push(name + '=' + value);
      return this;
    };
    this.build = function() {
      return { bodyString: this._pairs.join('&'), isForm: true };
    };
  }};

  _okhttp.Headers = { Builder: function() {
    this._headers = {};
    this.add = function(k, v) { this._headers[k] = String(v); return this; };
    this.set = function(k, v) { this._headers[k] = String(v); return this; };
    this.build = function() { return this._headers; };
  }};

  _okhttp.Request = { Builder: function() {
    this._url = '';
    this._method = 'GET';
    this._headers = {};
    this._body = null;
    this.url = function(u) { this._url = String(u); return this; };
    this.method = function(m, b) { this._method = String(m); if (b !== undefined && b !== null) this._body = b; return this; };
    this.addHeader = function(k, v) { this._headers[String(k)] = String(v); return this; };
    this.header = function(k, v) { this._headers[String(k)] = String(v); return this; };
    this.headers = function(h) { if (h && typeof h === 'object') Object.assign(this._headers, h); return this; };
    this.post = function(b) { this._method = 'POST'; if (b !== undefined) this._body = b; return this; };
    this.get = function() { this._method = 'GET'; return this; };
    this.build = function() {
      return { url: this._url, method: this._method, headers: this._headers, body: this._body };
    };
  }};

  _okhttp.OkHttpClient = function() {
    var self = this;
    this.newCall = function(request) {
      return {
        execute: function() {
          var headers = {};
          if (request.headers && typeof request.headers === 'object') {
            Object.keys(request.headers).forEach(function(k) {
              headers[String(k)] = String(request.headers[k]);
            });
          }
          var bodyStr = '';
          var bodyBase64 = null;
          var contentType = headers['Content-Type'] || headers['content-type'] || '';
          if (request.body) {
            if (request.body.bodyBase64 !== undefined) {
              bodyBase64 = String(request.body.bodyBase64);
              if (!contentType && request.body.mediaType && request.body.mediaType.type) {
                contentType = request.body.mediaType.type;
                headers['Content-Type'] = contentType;
              }
            } else if (request.body.content !== undefined) {
              bodyStr = String(request.body.content);
              if (!contentType && request.body.mediaType && request.body.mediaType.type) {
                contentType = request.body.mediaType.type;
                headers['Content-Type'] = contentType;
              }
            } else if (request.body.bodyString !== undefined) {
              bodyStr = request.body.bodyString;
              if (!contentType) { headers['Content-Type'] = 'application/x-www-form-urlencoded'; }
            }
          }
          var options = { method: String(request.method || 'GET'), headers: headers };
          if (bodyBase64 !== null) {
            options.bodyBase64 = bodyBase64;
          } else if (bodyStr !== '' || options.method !== 'GET') {
            options.body = bodyStr;
          }
          var spec = String(request.url || '') + ',' + JSON.stringify(options);
          var result = '';
          try { result = java.ajax(spec); } catch(e) { result = ''; }
          var responseHeaders = {};
          return {
            body: function() {
              return {
                string: function() { return result; },
                bytes: function() { return result; },
                charStream: function() { return result; },
                contentLength: function() { return result.length; }
              };
            },
            string: function() { return result; },
            headers: function() { return responseHeaders; },
            header: function(name) { return ''; },
            code: function() { return result !== '' ? 200 : 0; },
            isSuccessful: function() { return result !== ''; },
            isRedirect: function() { return false; },
            message: function() { return ''; },
            protocol: function() { return { toString: function() { return 'HTTP_2'; } }; }
          };
        }
      };
    };
    this.dispatcher = function() { return this; };
    this.cookieJar = function() { return this; };
  };

  _okhttp.Call = {};

  // Copy okhttp3 onto Packages.okhttp3
  globalThis.Packages.okhttp3 = _okhttp;
  // Also expose top-level constructors for JavaImporter compatibility
  globalThis.OkHttpClient = _okhttp.OkHttpClient;
  globalThis.Request = _okhttp.Request;
  globalThis.MediaType = _okhttp.MediaType;
  globalThis.RequestBody = _okhttp.RequestBody;
  globalThis.FormBody = _okhttp.FormBody;
  globalThis.Headers = _okhttp.Headers;
})();

java.base64DecodeToByteArray = function(input) {
  var base64 = java.__base64DecodeToByteArrayBase64(String(input || ''));
  return {
    __legadoByteArrayBase64: base64,
    toString: function() { return java.__base64DecodeToUtf8(base64); },
    valueOf: function() { return this.toString(); },
    toJSON: function() { return this.toString(); }
  };
};

// ── Hutool shims (merge into existing globals, don't overwrite) ──
(function() {
  // Only add hutool wrappers if they don't already exist
  if (typeof globalThis.DigestUtil === 'undefined') {
    globalThis.DigestUtil = {};
  }
  if (!globalThis.DigestUtil.md5Hex) {
    globalThis.DigestUtil.md5Hex = function(input) { return java.md5Encode(String(input)); };
  }
  globalThis.Packages.cn.hutool.crypto.digest = { DigestUtil: globalThis.DigestUtil };

  // Merge hutool StrUtil helpers into the existing StrUtil (which has .reverse from Rust)
  if (typeof globalThis.StrUtil === 'undefined') {
    globalThis.StrUtil = {};
  }
  if (!globalThis.StrUtil.format) {
    globalThis.StrUtil.format = function(fmt) {
      var args = Array.prototype.slice.call(arguments, 1);
      return fmt.replace(/\{\}/g, function() { return args.length ? String(args.shift()) : '{}'; });
    };
  }
  if (!globalThis.StrUtil.isEmpty) {
    globalThis.StrUtil.isEmpty = function(s) { return !s || String(s).trim() === ''; };
  }
  if (!globalThis.StrUtil.isNotEmpty) {
    globalThis.StrUtil.isNotEmpty = function(s) { return !globalThis.StrUtil.isEmpty(s); };
  }
  globalThis.Packages.cn.hutool.core.util = { StrUtil: globalThis.StrUtil };

  if (typeof globalThis.URLUtil === 'undefined') {
    globalThis.URLUtil = {
      encode: function(s) { return encodeURIComponent(String(s)); },
      decode: function(s) { try { return decodeURIComponent(String(s)); } catch(_) { return s; } }
    };
  }
  if (typeof globalThis.HexUtil === 'undefined') {
    globalThis.HexUtil = {
      decodeHex: function(hex) { return java.hexDecodeToString(String(hex)); },
      encodeHex: function(s) { return String(s).split('').map(function(c) { return c.charCodeAt(0).toString(16).padStart(2,'0'); }).join(''); },
      encodeHexStr: function(s) { return globalThis.HexUtil.encodeHex(s); }
    };
  }
  // Keep Base64 from Rust side (already has encode/decode)
  globalThis.Packages.cn.hutool.core.codec = { Base64: globalThis.Base64 };
})();

// ── JavaImporter — copies package properties into scope for `with()` ──
if (typeof globalThis._JavaImporterOriginal === 'undefined') {
  globalThis._JavaImporterOriginal = globalThis.JavaImporter;
}
globalThis.JavaImporter = function JavaImporter() {
  for (var i = 0; i < arguments.length; i++) {
    var pkg = arguments[i];
    if (pkg && typeof pkg === 'object') {
      for (var key in pkg) {
        if (pkg.hasOwnProperty(key) && typeof pkg[key] !== 'undefined') {
          this[key] = pkg[key];
        }
      }
    }
  }
  this.importPackage = function(pkg) {
    if (pkg && typeof pkg === 'object') {
      for (var key in pkg) {
        if (pkg.hasOwnProperty(key) && typeof pkg[key] !== 'undefined') {
          this[key] = pkg[key];
        }
      }
    }
    return this;
  };
  return this;
};
if (typeof globalThis.importPackage === 'undefined') {
  globalThis.importPackage = function() { return true; };
}

// ── source API layer ──
source.getLoginInfoMap = function() {
  const owner = this;
  return {
    get(key) {
      return owner.__getLoginInfoValue(String(key));
    },
    set(values) {
      if (!values || typeof values !== 'object') {
        return false;
      }
      if (values && typeof values === 'object') {
        Object.keys(values).forEach((key) => owner.__setLoginInfoValue(String(key), values[key]));
      }
      return true;
    },
    save() {
      return true;
    },
    toJSON() {
      try {
        return JSON.parse(owner.__getLoginInfoJson() || '{}');
      } catch (_) {
        return {};
      }
    },
    toString() {
      return JSON.stringify(this.toJSON());
    }
  };
};
source.putLoginInfo = function(values, value) {
  if (arguments.length >= 2) {
    return this.__setLoginInfoValue(String(values), value);
  }
  if (typeof values === 'string') {
    try {
      values = JSON.parse(values);
    } catch (_) {
      return false;
    }
  }
  if (!values || typeof values !== 'object') {
    return false;
  }
  return this.getLoginInfoMap().set(values);
};
source.removeLoginHeader = function() {
  return this.__clearLoginInfo();
};
source.refreshExplore = function() { return false; };
java.ajaxAll = function(specs) {
  const list = Array.isArray(specs) ? specs : [];
  return JSON.parse(java.__ajaxAll(JSON.stringify(list))).map(function(text) {
    return {
      body: function() { return text; },
      string: function() { return text; },
      toString: function() { return text; }
    };
  });
};
"#,
    )
    .map_err(|err| anyhow::anyhow!("JS Exception: {}", catch_js_message(&ctx, &err)))?;
    Ok(())
}

fn source_variable_key(source_key: &str, key: Option<&str>) -> String {
    match key.filter(|value| !value.trim().is_empty()) {
        Some(key) => format!("__source_var_{}::{}", source_key, key),
        None => format!("__source_var_{}::__default", source_key),
    }
}

fn source_login_prefix(source_key: &str) -> String {
    format!("__source_login_{}::", source_key)
}

fn source_login_key(source_key: &str, key: &str) -> String {
    format!("{}{}", source_login_prefix(source_key), key)
}

fn java_storage_key(key: &str) -> String {
    format!("__java_kv_{}", key)
}

fn cookie_key(domain: &str, name: Option<&str>) -> String {
    match name.filter(|value| !value.trim().is_empty()) {
        Some(name) => format!("__cookie_{}::{}", domain, name),
        None => format!("__cookie_{}", domain),
    }
}

fn get_cookie_value(domain: &str, name: Option<&str>) -> String {
    let map = JS_KV.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(name) = name.filter(|value| !value.trim().is_empty()) {
        if let Some(value) = map.get(&cookie_key(domain, Some(name))) {
            return value.clone();
        }
    }
    map.get(&cookie_key(domain, None))
        .cloned()
        .unwrap_or_default()
}

fn is_http_url(value: &str) -> bool {
    value.starts_with("http://") || value.starts_with("https://")
}

fn java_get_string(input: &str, path: &str) -> String {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    let Ok(value) = serde_json::from_str::<JsonValue>(trimmed) else {
        return String::new();
    };
    let path = if path.trim_start().starts_with('$') {
        path.to_string()
    } else {
        format!("$.{}", path.trim_start_matches('.'))
    };
    crate::parser::jsonpath::jsonpath_first_string(&value, &path).unwrap_or_default()
}

fn java_aes_base64_decode_to_string(input: &str, key: &str, algorithm: &str, iv: &str) -> String {
    let algorithm = algorithm.to_ascii_uppercase();
    if algorithm != "AES/CBC/PKCS5PADDING" && algorithm != "AES/CBC/PKCS7PADDING" {
        return String::new();
    }

    let Ok(mut encrypted) = base64::engine::general_purpose::STANDARD.decode(input.trim()) else {
        return String::new();
    };

    let Ok(cipher) = Aes128CbcDecryptor::new_from_slices(key.as_bytes(), iv.as_bytes()) else {
        return String::new();
    };

    cipher
        .decrypt_padded_mut::<Pkcs7>(&mut encrypted)
        .ok()
        .and_then(|bytes| String::from_utf8(bytes.to_vec()).ok())
        .unwrap_or_default()
}

fn decode_hex_to_utf8_string(hex: &str) -> String {
    let hex = hex.trim_start_matches("0x").trim_start_matches("0X");
    let bytes = (0..hex.len())
        .step_by(2)
        .filter_map(|i| u8::from_str_radix(&hex[i..(i + 2).min(hex.len())], 16).ok())
        .collect::<Vec<_>>();
    String::from_utf8(bytes)
        .unwrap_or_else(|err| String::from_utf8_lossy(err.as_bytes()).into_owned())
}

fn js_source_arg_literal(arg: &JsSourceArg) -> anyhow::Result<String> {
    Ok(match arg {
        JsSourceArg::String(value) => serde_json::to_string(value)?,
        JsSourceArg::Int(value) => value.to_string(),
        JsSourceArg::Float(value) if value.is_finite() => value.to_string(),
        JsSourceArg::Float(_) => "null".to_string(),
        JsSourceArg::Bool(value) => value.to_string(),
        JsSourceArg::Json(value) => serde_json::to_string(value)?,
        JsSourceArg::Null => "null".to_string(),
    })
}

fn is_safe_js_identifier(value: &str) -> bool {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !(first == '_' || first == '$' || first.is_ascii_alphabetic()) {
        return false;
    }
    chars.all(|ch| ch == '_' || ch == '$' || ch.is_ascii_alphanumeric())
}

fn eval_script<'js>(ctx: rquickjs::Ctx<'js>, script: &str) -> anyhow::Result<Value<'js>> {
    // Legado 书源脚本沿用 Rhino 非严格语义，常见未声明全局赋值（chapters = ...），
    // rquickjs eval 为严格模式会抛 "x is not defined"。var 补全必须在首次 eval 之前：
    // 首次执行失败时，脚本里已执行的顶层 let/const 已进入该 Context 的全局词法环境，
    // 同一 Context 内重试会因 redeclaration 立即失败（这正是旧版"失败后重试"永远
    // 修不好"let + 未声明赋值"组合脚本的原因）。
    let script = prepare_legacy_js_script(script);
    let prepared = prepend_undeclared_vars(&script);
    if prepared != script {
        match ctx.eval(&*prepared) {
            Ok(v) => return Ok(v),
            Err(err) => {
                let msg = catch_js_message(&ctx, &err);
                // 补全名单误判与脚本内 let/const 冲突时是解析期 redeclaration 错误，
                // 解析失败不会执行任何代码、Context 仍干净，可安全回退原脚本。
                if msg.contains("redeclar") {
                    return ctx.eval(&*script).map_err(|e2| {
                        anyhow::anyhow!("JS Exception: {}", catch_js_message(&ctx, &e2))
                    });
                }
                return Err(anyhow::anyhow!("JS Exception: {}", msg));
            }
        }
    }
    ctx.eval(&*script)
        .map_err(|err| anyhow::anyhow!("JS Exception: {}", catch_js_message(&ctx, &err)))
}

fn catch_js_message(ctx: &rquickjs::Ctx<'_>, err: &rquickjs::Error) -> String {
    if let Some(ex) = ctx.catch().into_exception() {
        let message = ex.message().unwrap_or_else(|| err.to_string());
        if let Some(stack) = ex.stack().filter(|value| !value.trim().is_empty()) {
            return format!("{message}\n{stack}");
        }
        return message;
    }
    err.to_string()
}

fn leading_identifier(text: &str) -> String {
    let mut ident = String::new();
    for ch in text.trim_start().chars() {
        if ident.is_empty() {
            if is_identifier_start(ch) {
                ident.push(ch);
                continue;
            }
            break;
        }
        if is_identifier_continue(ch) {
            ident.push(ch);
            continue;
        }
        break;
    }
    ident
}

fn prepare_legacy_js_script(script: &str) -> String {
    // Rhino permits `with(javaImport) { ... }`; QuickJS strict eval rejects `with` at parse time.
    // The JavaImporter shim exposes imported helpers as globals, so unwrapping the block preserves
    // Rhino's practical top-level function visibility used by Legado book-source scripts.
    static THIS_DESTRUCTURE_RE: Lazy<regex::Regex> =
        Lazy::new(|| regex::Regex::new(r"\b(const|let|var)\s+\{([^}]*)\}\s*=\s*this\s*;").unwrap());
    let script = strip_with_wrappers(script);
    let script = THIS_DESTRUCTURE_RE
        .replace_all(&script, "$1 {$2} = globalThis;")
        .into_owned();
    replace_this_member_outside_strings(&script)
}

fn strip_with_wrappers(script: &str) -> String {
    static WITH_RE: Lazy<regex::Regex> =
        Lazy::new(|| regex::Regex::new(r"\bwith\s*\([^)]*\)\s*\{").unwrap());

    let mut output = String::with_capacity(script.len());
    let mut pos = 0usize;
    while let Some(open) = WITH_RE.find_at(script, pos) {
        output.push_str(&script[pos..open.start()]);
        let open_brace = open.end() - 1;
        let Some(close_brace) = find_matching_brace(script, open_brace) else {
            output.push_str(&script[open.start()..]);
            return output;
        };
        output.push_str(&strip_with_wrappers(&script[open.end()..close_brace]));
        pos = close_brace + 1;
    }
    output.push_str(&script[pos..]);
    output
}

fn find_matching_brace(script: &str, open_brace: usize) -> Option<usize> {
    let mut depth = 0usize;
    let mut quote: Option<char> = None;
    let mut escaped = false;
    let mut line_comment = false;
    let mut block_comment = false;

    for (idx, ch) in script[open_brace..].char_indices() {
        let idx = open_brace + idx;
        if line_comment {
            if ch == '\n' {
                line_comment = false;
            }
            continue;
        }
        if block_comment {
            if ch == '/' && script[..idx].ends_with('*') {
                block_comment = false;
            }
            continue;
        }
        if let Some(q) = quote {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == q {
                quote = None;
            }
            continue;
        }

        if ch == '/' {
            let rest = &script[idx..];
            if rest.starts_with("//") {
                line_comment = true;
                continue;
            }
            if rest.starts_with("/*") {
                block_comment = true;
                continue;
            }
        }
        if matches!(ch, '"' | '\'' | '`') {
            quote = Some(ch);
            continue;
        }
        match ch {
            '{' => depth += 1,
            '}' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Some(idx);
                }
            }
            _ => {}
        }
    }
    None
}

fn inline_source_login_url(script: &str, login_url: Option<&str>) -> String {
    let Some(login_url) = login_url.filter(|value| !value.trim().is_empty()) else {
        return script.to_string();
    };
    static LOGIN_EVAL_RE: Lazy<regex::Regex> = Lazy::new(|| {
        regex::Regex::new(r"eval\s*\(\s*String\s*\(\s*source\.loginUrl\s*\)\s*\)\s*;?").unwrap()
    });
    if !LOGIN_EVAL_RE.is_match(script) {
        return script.to_string();
    }
    let login_url = prepare_legacy_js_script(login_url);
    let replacement = format!("{login_url}\n");
    LOGIN_EVAL_RE
        .replace_all(script, regex::NoExpand(&replacement))
        .into_owned()
}

fn replace_this_member_outside_strings(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut i = 0usize;
    let mut quote: Option<char> = None;
    let mut escaped = false;
    let mut line_comment = false;
    let mut block_comment = false;

    while i < input.len() {
        let rest = &input[i..];
        if line_comment {
            let ch = rest.chars().next().unwrap();
            output.push(ch);
            i += ch.len_utf8();
            if ch == '\n' {
                line_comment = false;
            }
            continue;
        }
        if block_comment {
            if rest.starts_with("*/") {
                output.push_str("*/");
                i += 2;
                block_comment = false;
            } else {
                let ch = rest.chars().next().unwrap();
                output.push(ch);
                i += ch.len_utf8();
            }
            continue;
        }
        if let Some(current_quote) = quote {
            let ch = rest.chars().next().unwrap();
            output.push(ch);
            i += ch.len_utf8();
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == current_quote {
                quote = None;
            }
            continue;
        }

        if rest.starts_with("//") {
            output.push_str("//");
            i += 2;
            line_comment = true;
            continue;
        }
        if rest.starts_with("/*") {
            output.push_str("/*");
            i += 2;
            block_comment = true;
            continue;
        }
        let ch = rest.chars().next().unwrap();
        if matches!(ch, '\'' | '"' | '`') {
            output.push(ch);
            i += ch.len_utf8();
            quote = Some(ch);
            continue;
        }
        if rest.starts_with("this.") {
            output.push_str("globalThis.");
            i += "this.".len();
            continue;
        }
        output.push(ch);
        i += ch.len_utf8();
    }

    output
}

/// Detect undeclared top-level variable assignments (Legado convention)
/// and prepend `var` declarations so they work in strict-mode rquickjs.
///
/// 已用 var/let/const/function 声明过的名字必须排除，否则补出的 `var x;`
/// 会与脚本内 `let x` 冲突，整段脚本解析失败。
fn prepend_undeclared_vars(script: &str) -> String {
    let mut declared = std::collections::BTreeSet::new();
    let mut assigned = std::collections::BTreeSet::new();
    for line in script.lines() {
        let trimmed = line.trim();
        let starts_at_column_zero = line == trimmed;
        if trimmed.is_empty() || trimmed.starts_with("//") {
            continue;
        }
        if starts_at_column_zero {
            if let Some(rest) = trimmed
                .strip_prefix("var ")
                .or_else(|| trimmed.strip_prefix("let "))
                .or_else(|| trimmed.strip_prefix("const "))
            {
                // `let a = 1, b = 2` 记录 a、b；表达式内逗号会带来多余候选名，
                // 只会让 declared 集合偏大（少补 var），不会引入 redeclaration。
                for part in rest.split(',') {
                    let name = leading_identifier(part);
                    if !name.is_empty() {
                        declared.insert(name);
                    }
                }
                continue;
            }
            if let Some(rest) = trimmed.strip_prefix("function ") {
                let name = leading_identifier(rest);
                if !name.is_empty() {
                    declared.insert(name);
                }
                continue;
            }
        }
        if let Some(name) = legacy_for_loop_identifier(trimmed) {
            assigned.insert(name);
        }
        // Check for bare identifier assignment at line start
        if let Some(eq_pos) = trimmed.find('=') {
            // 跳过 ==、===、=> 等非赋值形态
            if matches!(trimmed[eq_pos + 1..].chars().next(), Some('=') | Some('>')) {
                continue;
            }
            let candidate = trimmed[..eq_pos].trim();
            if is_valid_identifier(candidate) && !is_keyword(candidate) {
                assigned.insert(candidate.to_string());
            }
        }
    }
    let names: Vec<String> = assigned.difference(&declared).cloned().collect();
    if names.is_empty() {
        return script.to_string();
    }
    format!("var {};\n{}", names.join(", "), script)
}

fn legacy_for_loop_identifier(line: &str) -> Option<String> {
    let mut rest = line.strip_prefix("for")?.trim_start();
    rest = rest.strip_prefix('(')?.trim_start();
    if rest.starts_with("let ") || rest.starts_with("var ") || rest.starts_with("const ") {
        return None;
    }
    let name = leading_identifier(rest);
    if name.is_empty() {
        return None;
    }
    let after = rest[name.len()..].trim_start();
    if after.starts_with("in ") || after.starts_with("of ") || after.starts_with('=') {
        return Some(name);
    }
    None
}

fn is_valid_identifier(s: &str) -> bool {
    let mut chars = s.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !is_identifier_start(first) {
        return false;
    }
    chars.all(is_identifier_continue)
}

fn is_identifier_start(ch: char) -> bool {
    ch == '_' || ch == '$' || ch.is_alphabetic()
}

fn is_identifier_continue(ch: char) -> bool {
    is_identifier_start(ch) || ch.is_ascii_digit() || ch.is_numeric()
}

fn is_keyword(s: &str) -> bool {
    matches!(
        s,
        "if" | "else"
            | "for"
            | "while"
            | "do"
            | "switch"
            | "case"
            | "break"
            | "continue"
            | "return"
            | "throw"
            | "try"
            | "catch"
            | "finally"
            | "new"
            | "delete"
            | "typeof"
            | "instanceof"
            | "in"
            | "of"
            | "this"
            | "super"
            | "class"
            | "function"
            | "var"
            | "let"
            | "const"
            | "import"
            | "export"
            | "default"
            | "void"
            | "yield"
            | "async"
            | "await"
            | "true"
            | "false"
            | "null"
            | "undefined"
    )
}

fn active_js_lib_script(active_context: &ActiveJsContext) -> anyhow::Result<String> {
    let Some(js_lib) = active_context
        .js_lib
        .clone()
        .filter(|value| !value.trim().is_empty())
    else {
        return Ok(String::new());
    };
    let cache_key = md5_hex(&js_lib);
    if let Some(cached) = JS_LIB_CACHE
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get(&cache_key)
        .cloned()
    {
        return Ok(cached);
    }

    let compiled = compile_js_lib(&js_lib)?;
    JS_LIB_CACHE
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .insert(cache_key, compiled.clone());
    Ok(compiled)
}

fn compile_js_lib(js_lib: &str) -> anyhow::Result<String> {
    let trimmed = js_lib.trim();
    if trimmed.is_empty() {
        return Ok(String::new());
    }
    if trimmed.starts_with('{') {
        if let Ok(value) = serde_json::from_str::<JsonValue>(trimmed) {
            if let Some(map) = value.as_object() {
                let mut scripts = Vec::new();
                for entry in map.values() {
                    if let Some(raw) = entry.as_str() {
                        scripts.push(resolve_js_lib_entry(raw)?);
                    }
                }
                return Ok(scripts.join("\n"));
            }
        }
    }
    Ok(trimmed.to_string())
}

fn resolve_js_lib_entry(entry: &str) -> anyhow::Result<String> {
    let value = entry.trim();
    if value.starts_with("http://") || value.starts_with("https://") {
        return send_text_blocking(JS_HTTP_CLIENT.get(value));
    }
    Ok(value.to_string())
}

fn java_format_to_chrono(java_fmt: &str) -> String {
    java_fmt
        .replace("yyyy", "%Y")
        .replace("yy", "%y")
        .replace("MM", "%m")
        .replace("dd", "%d")
        .replace("HH", "%H")
        .replace("hh", "%I")
        .replace("mm", "%M")
        .replace("ss", "%S")
        .replace("SSS", "%3f")
        .replace("EEEE", "%A")
        .replace("EEE", "%a")
}

fn java_time_format(timestamp: i64) -> String {
    let secs = if timestamp > 1_000_000_000_000 {
        timestamp / 1000
    } else {
        timestamp
    };
    match Local.timestamp_opt(secs, 0).single() {
        Some(dt) => dt.format("%Y-%m-%d %H:%M").to_string(),
        None => String::new(),
    }
}

fn java_ajax(spec: &str) -> anyhow::Result<String> {
    let (url, options) = split_ajax_spec(spec);
    if url.trim().is_empty() {
        return Ok(String::new());
    }

    let options_json = options
        .and_then(|raw| serde_json::from_str::<JsonValue>(raw).ok())
        .unwrap_or(JsonValue::Null);

    let method = options_json
        .get("method")
        .and_then(|v| v.as_str())
        .unwrap_or("GET")
        .to_uppercase();
    let method = Method::from_bytes(method.as_bytes()).unwrap_or(Method::GET);

    let mut req = JS_HTTP_CLIENT.request(method, url.trim());

    if let Some(headers) = options_json.get("headers").and_then(|v| v.as_object()) {
        for (key, value) in headers {
            if let Some(value) = value.as_str() {
                req = req.header(key, value);
            } else if !value.is_null() {
                req = req.header(key, value.to_string());
            }
        }
    }

    if let Some(body_base64) = options_json
        .get("bodyBase64")
        .or_else(|| options_json.get("bodyBytesBase64"))
        .and_then(|v| v.as_str())
    {
        if let Some(bytes) = decode_base64_bytes(body_base64) {
            req = req.body(bytes);
        }
    } else if let Some(body) = options_json.get("body") {
        if let Some(body) = body.as_str() {
            req = req.body(body.to_string());
        } else if !body.is_null() {
            req = req.body(body.to_string());
        }
    }

    send_text_blocking(req)
}

fn decode_base64_bytes(input: &str) -> Option<Vec<u8>> {
    let compact: String = input
        .chars()
        .filter(|ch| !ch.is_ascii_whitespace())
        .collect();
    if compact.is_empty() {
        return Some(Vec::new());
    }
    base64::engine::general_purpose::STANDARD
        .decode(compact.as_bytes())
        .or_else(|_| base64::engine::general_purpose::STANDARD_NO_PAD.decode(compact.as_bytes()))
        .or_else(|_| base64::engine::general_purpose::URL_SAFE.decode(compact.as_bytes()))
        .or_else(|_| base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(compact.as_bytes()))
        .ok()
}

fn decode_base64_to_utf8(input: &str) -> Option<String> {
    decode_base64_bytes(input).map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
}

fn java_request_simple(method: &str, url: &str, body: Option<String>) -> anyhow::Result<String> {
    let method = Method::from_bytes(method.as_bytes()).unwrap_or(Method::GET);
    let mut req = JS_HTTP_CLIENT.request(method, url.trim());
    if let Some(body) = body {
        req = req.body(body);
    }
    send_text_blocking(req)
}

fn legado_http_request(
    method: &str,
    url: &str,
    body: Option<String>,
    headers: Option<Value<'_>>,
) -> anyhow::Result<String> {
    let method = Method::from_bytes(method.as_bytes()).unwrap_or(Method::GET);
    let mut req = JS_HTTP_CLIENT.request(method, url.trim());
    req = apply_header_js_value(req, headers);
    if let Some(body) = body {
        req = req.body(body);
    }
    send_text_blocking(req)
}

fn js_callback_arg_to_string(value: Value<'_>) -> String {
    if value.is_null() || value.is_undefined() {
        return String::new();
    }
    if let Some(s) = value.clone().into_string() {
        return s
            .to_string()
            .map(|value| value.to_string())
            .unwrap_or_default();
    }
    let ctx = value.ctx().clone();
    match ctx.json_stringify(value) {
        Ok(Some(json)) => json.to_string().unwrap_or_default(),
        _ => String::new(),
    }
}

fn apply_header_js_value(
    req: reqwest::blocking::RequestBuilder,
    headers: Option<Value<'_>>,
) -> reqwest::blocking::RequestBuilder {
    let Some(headers) = headers.filter(|value| !value.is_null() && !value.is_undefined()) else {
        return req;
    };
    if let Some(raw) = headers.clone().into_string() {
        let raw = raw.to_string().unwrap_or_default();
        return apply_header_json(req, Some(raw.as_str()));
    }
    let ctx = headers.ctx().clone();
    let text = match ctx.json_stringify(headers) {
        Ok(Some(json)) => json.to_string().unwrap_or_default(),
        _ => String::new(),
    };
    apply_header_json(req, Some(text.as_str()))
}

fn legado_http_request_options(options: &str) -> anyhow::Result<String> {
    let value = serde_json::from_str::<JsonValue>(options).unwrap_or(JsonValue::Null);
    let url = value
        .get("url")
        .or_else(|| value.get("href"))
        .and_then(|item| item.as_str())
        .unwrap_or_default();
    if url.trim().is_empty() {
        return Ok(String::new());
    }

    let method = value
        .get("method")
        .and_then(|item| item.as_str())
        .unwrap_or("GET")
        .to_uppercase();
    let method = Method::from_bytes(method.as_bytes()).unwrap_or(Method::GET);
    let mut req = JS_HTTP_CLIENT.request(method, url.trim());
    if let Some(headers) = value.get("headers") {
        req = apply_header_value(req, headers);
    }
    if let Some(body) = value.get("body") {
        if let Some(body) = body.as_str() {
            req = req.body(body.to_string());
        } else if !body.is_null() {
            req = req.body(body.to_string());
        }
    }
    send_text_blocking(req)
}

fn apply_header_json(
    mut req: reqwest::blocking::RequestBuilder,
    headers: Option<&str>,
) -> reqwest::blocking::RequestBuilder {
    let Some(headers) = headers.filter(|value| !value.trim().is_empty()) else {
        return req;
    };
    let Ok(value) = serde_json::from_str::<JsonValue>(headers) else {
        return req;
    };
    req = apply_header_value(req, &value);
    req
}

fn apply_header_value(
    mut req: reqwest::blocking::RequestBuilder,
    headers: &JsonValue,
) -> reqwest::blocking::RequestBuilder {
    if let Some(headers) = headers.as_object() {
        for (key, value) in headers {
            if let Some(value) = value.as_str() {
                req = req.header(key, value);
            } else if !value.is_null() {
                req = req.header(key, value.to_string());
            }
        }
    }
    req
}

fn split_ajax_spec(spec: &str) -> (&str, Option<&str>) {
    let mut depth = 0i32;
    let mut in_string = false;
    let mut quote = '\0';
    let mut escaped = false;

    for (idx, ch) in spec.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }

        match ch {
            '\\' if in_string => {
                escaped = true;
            }
            '"' | '\'' if in_string && ch == quote => {
                in_string = false;
                quote = '\0';
            }
            '"' | '\'' if !in_string => {
                in_string = true;
                quote = ch;
            }
            '{' | '[' if !in_string => depth += 1,
            '}' | ']' if !in_string => depth -= 1,
            ',' if !in_string && depth == 0 => {
                let left = &spec[..idx];
                let right = &spec[idx + ch.len_utf8()..];
                return (left, Some(right.trim()));
            }
            _ => {}
        }
    }

    (spec, None)
}
