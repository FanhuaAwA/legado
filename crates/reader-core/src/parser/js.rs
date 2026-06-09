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
use std::collections::HashMap;
use std::sync::Mutex;
use uuid::Uuid;

static JS_KV: Lazy<Mutex<HashMap<String, String>>> = Lazy::new(|| Mutex::new(HashMap::new()));
static JS_LIB_CACHE: Lazy<Mutex<HashMap<String, String>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));
static JS_HTTP_CLIENT: Lazy<Client> = Lazy::new(|| {
    Client::builder()
        .cookie_store(true)
        .gzip(true)
        .brotli(true)
        .deflate(true)
        .build()
        .expect("failed to build JS HTTP client")
});
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
thread_local! {
    static ACTIVE_JS_LIB: RefCell<Option<String>> = const { RefCell::new(None) };
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
    ACTIVE_JS_LIB.with(|cell| {
        let previous = cell.replace(js_lib.map(|value| value.to_string()));
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
    let rt = Runtime::new()?;
    let ctx = Context::full(&rt)?;
    ctx.with(|ctx| {
        let globals = ctx.globals();
        let input_value = input.unwrap_or("");
        let base_url_value = base_url.unwrap_or("");
        let shared_js = active_js_lib_script()?;

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
        let sk_clone = source_key_val.clone();
        source_obj.set("key", source_key_val.clone())?;
        source_obj.set("getKey", Func::new(move || sk_clone.clone()))?;

        let sk_for_login = source_key_val.clone();
        source_obj.set(
            "getLoginInfoMap",
            Func::new(move || -> String {
                let map = JS_KV.lock().unwrap_or_else(|e| e.into_inner());
                let prefix = format!("__source_login_{}", sk_for_login);
                let items: Vec<String> = map
                    .iter()
                    .filter(|(k, _)| k.starts_with(&prefix))
                    .map(|(k, v)| format!("{}={}", k.replacen(&prefix, "", 1).trim_start_matches('_'), v))
                    .collect();
                items.join("\n")
            }),
        )?;

        let sk_for_var = source_key_val.clone();
        source_obj.set(
            "getVariable",
            Func::new(move |key: String| -> Option<String> {
                let map = JS_KV.lock().unwrap_or_else(|e| e.into_inner());
                let full_key = format!("__source_var_{}_{}", sk_for_var, key);
                map.get(&full_key).cloned()
            }),
        )?;
        let sk_for_set = source_key_val.clone();
        source_obj.set(
            "setVariable",
            Func::new(move |key: String, val: String| -> bool {
                let mut map = JS_KV.lock().unwrap_or_else(|e| e.into_inner());
                let full_key = format!("__source_var_{}_{}", sk_for_set, key);
                map.insert(full_key, val);
                true
            }),
        )?;
        let sk_for_put = source_key_val.clone();
        source_obj.set(
            "putVariable",
            Func::new(move |key: String, val: String| -> bool {
                let mut map = JS_KV.lock().unwrap_or_else(|e| e.into_inner());
                let full_key = format!("__source_var_{}_{}", sk_for_put, key);
                map.insert(full_key, val);
                true
            }),
        )?;
        source_obj.set(
            "putLoginInfo",
            Func::new(|key: String, val: String| -> bool {
                let mut map = JS_KV.lock().unwrap_or_else(|e| e.into_inner());
                map.insert(format!("__source_login_{}", key), val);
                true
            }),
        )?;
        globals.set("source", source_obj)?;

        let cookie_obj = Object::new(ctx.clone())?;
        cookie_obj.set(
            "getCookie",
            Func::new(|key: String| -> Option<String> {
                let map = JS_KV.lock().unwrap_or_else(|e| e.into_inner());
                map.get(&format!("__cookie_{}", key)).cloned()
            }),
        )?;
        cookie_obj.set(
            "setCookie",
            Func::new(|key: String, val: String| -> bool {
                let mut map = JS_KV.lock().unwrap_or_else(|e| e.into_inner());
                map.insert(format!("__cookie_{}", key), val);
                true
            }),
        )?;
        cookie_obj.set(
            "removeCookie",
            Func::new(|key: String| -> String {
                let mut map = JS_KV.lock().unwrap_or_else(|e| e.into_inner());
                map.remove(&format!("__cookie_{}", key));
                "".to_string()
            }),
        )?;
        cookie_obj.set(
            "getKey",
            Func::new(|key: String| -> Option<String> {
                let map = JS_KV.lock().unwrap_or_else(|e| e.into_inner());
                map.get(&format!("__cookie_{}", key)).cloned()
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
            Func::new(|key: String, val: String| -> bool {
                let mut map = JS_KV.lock().unwrap_or_else(|e| e.into_inner());
                map.insert(key, val);
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
            Func::new(|key: String, val: String| -> bool {
                let mut map = JS_KV.lock().unwrap_or_else(|e| e.into_inner());
                map.insert(key, val);
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
            Func::new(|input: String| -> String {
                // Simple verification code: hash the input with some fixed salt
                let code = md5_hex(&format!("fanqie_verify_{input}_salt"));
                code[..8].to_string()
            }),
        )?;
        java_obj.set(
            "base64DecodeToByteArray",
            Func::new(|input: String| -> String {
                base64::engine::general_purpose::STANDARD
                    .decode(input)
                    .map(|bytes| bytes.iter().map(|b| *b as char).collect::<String>())
                    .unwrap_or_default()
            }),
        )?;
        java_obj.set(
            "toast",
            Func::new(|msg: String| -> bool {
                tracing::info!(target: "reader_core::js_source::toast", "{msg}");
                true
            }),
        )?;
        java_obj.set(
            "longToast",
            Func::new(|msg: String| -> bool {
                tracing::info!(target: "reader_core::js_source::toast", "long: {msg}");
                true
            }),
        )?;
        java_obj.set(
            "log",
            Func::new(|msg: String| -> bool {
                tracing::info!(target: "reader_core::js_source::log", "{msg}");
                true
            }),
        )?;
        java_obj.set(
            "androidId",
            Func::new(|| -> String { JS_DEVICE_ID.clone() }),
        )?;
        java_obj.set("deviceID", Func::new(|| -> String { JS_DEVICE_ID.clone() }))?;
        java_obj.set(
            "get",
            Func::new(|url: String| -> String {
                java_request_simple("GET", &url, None).unwrap_or_default()
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
            Func::new(|url: String, body: String| -> String {
                java_request_simple("PUT", &url, Some(body)).unwrap_or_default()
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
                let hex = hex.trim_start_matches("0x").trim_start_matches("0X");
                (0..hex.len())
                    .step_by(2)
                    .filter_map(|i| u8::from_str_radix(&hex[i..(i + 2).min(hex.len())], 16).ok())
                    .map(|b| b as char)
                    .collect::<String>()
            }),
        )?;
        java_obj.set(
            "ajaxAll",
            Func::new(|specs: rquickjs::Array<'_>| -> String {
                let mut results: Vec<String> = Vec::new();
                for i in 0..specs.len() {
                    let spec = specs.get::<String>(i).unwrap_or_default();
                    let result = java_ajax(&spec).unwrap_or_default();
                    results.push(result);
                }
                format!("[{}]", results.join(","))
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
            Func::new(|message: String| -> bool {
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

        globals.set("book", Object::new(ctx.clone())?)?;
        globals.set("chapter", Object::new(ctx.clone())?)?;
        globals.set("title", "")?;
        globals.set("nextChapterUrl", "")?;
        globals.set("rssArticle", Object::new(ctx.clone())?)?;

        if let Some(bindings) = bindings {
            for (key, value) in bindings {
                let js_value = ctx.json_parse(value.to_string())?;
                globals.set(key.as_str(), js_value)?;
            }
        }

        if !shared_js.trim().is_empty() {
            eval_script(ctx.clone(), &shared_js)?;
        }

        let v = eval_script(ctx.clone(), script)?;
        js_value_to_string(ctx, v)
    })
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
    match ctx.eval(script) {
        Ok(v) => Ok(v),
        Err(first_err) => {
            // rquickjs eval runs in strict mode, but Legado book source scripts
            // often use undeclared global variables (Rhino allows this).
            // Collect exception info, then retry with var declarations prepended.
            let first_msg = ctx
                .catch()
                .into_exception()
                .and_then(|ex| ex.message())
                .unwrap_or_else(|| first_err.to_string());

            let fixed = prepend_undeclared_vars(script);
            if fixed != script {
                match ctx.eval(&*fixed) {
                    Ok(v) => return Ok(v),
                    Err(_) => {} // fall through
                }
            }
            Err(anyhow::anyhow!("JS Exception: {}", first_msg))
        }
    }
}

/// Detect undeclared top-level variable assignments (Legado convention)
/// and prepend `var` declarations so they work in strict-mode rquickjs.
fn prepend_undeclared_vars(script: &str) -> String {
    let mut names = std::collections::BTreeSet::new();
    for line in script.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with("//") {
            continue;
        }
        // Match: identifier =  (but not var/let/const identifier =)
        if let Some(rest) = trimmed
            .strip_prefix("var ")
            .or_else(|| trimmed.strip_prefix("let "))
            .or_else(|| trimmed.strip_prefix("const "))
        {
            // Extract the declared name to avoid double-declaring
            if let Some(_name) = rest.split(|c: char| !c.is_ascii_alphanumeric() && c != '_' && c != '$').next() {
                // Already declared, don't add
                continue;
            }
        }
        // Check for bare identifier assignment at line start
        if let Some(eq_pos) = trimmed.find('=') {
            let candidate = trimmed[..eq_pos].trim();
            if is_valid_identifier(candidate) && !is_keyword(candidate) {
                names.insert(candidate.to_string());
            }
        }
    }
    if names.is_empty() {
        return script.to_string();
    }
    let decls: Vec<String> = names.into_iter().collect();
    format!("var {};\n{}", decls.join(", "), script)
}

fn is_valid_identifier(s: &str) -> bool {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' || c == '$' => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '$')
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

fn active_js_lib_script() -> anyhow::Result<String> {
    let js_lib = ACTIVE_JS_LIB.with(|cell| cell.borrow().clone());
    let Some(js_lib) = js_lib.filter(|value| !value.trim().is_empty()) else {
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
        let response = JS_HTTP_CLIENT.get(value).send()?;
        return Ok(response.text().unwrap_or_default());
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

    if let Some(body) = options_json.get("body") {
        if let Some(body) = body.as_str() {
            req = req.body(body.to_string());
        } else if !body.is_null() {
            req = req.body(body.to_string());
        }
    }

    let response = req.send()?;
    Ok(response.text().unwrap_or_default())
}

fn java_request_simple(method: &str, url: &str, body: Option<String>) -> anyhow::Result<String> {
    let method = Method::from_bytes(method.as_bytes()).unwrap_or(Method::GET);
    let mut req = JS_HTTP_CLIENT.request(method, url.trim());
    if let Some(body) = body {
        req = req.body(body);
    }
    let response = req.send()?;
    Ok(response.text().unwrap_or_default())
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
    let response = req.send()?;
    Ok(response.text().unwrap_or_default())
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
    let response = req.send()?;
    Ok(response.text().unwrap_or_default())
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
