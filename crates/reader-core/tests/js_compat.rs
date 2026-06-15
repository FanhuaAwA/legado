use axum::{
    body::Bytes,
    response::Json,
    routing::{get, post},
    Router,
};
use reader_core::parser::js::{eval_js, set_js_engine_timeout_secs, with_js_source};
use reader_core::{ReaderCore, ReaderCoreOptions};
use serde_json::json;
use std::future::Future;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

#[test]
fn java_aes_base64_decode_to_string_decrypts_legado_paths() {
    let encrypted = "UhQTfQq/qXGCKPd5D+cjxB7Y0AzwiFMYBmcN5nIm2PboUavKiWEIVaAPIhDXbkox";
    let result = eval_js(
        r#"java.aesBase64DecodeToString(result, "f041c49714d39908", "AES/CBC/PKCS5Padding", "0123456789abcdef")"#,
        encrypted,
        "http://api.jmlldsc.com",
    )
    .unwrap();

    assert_eq!(result, "http://api.lemiyigou.com/655/655791/70398.json");
}

/// 回归：顶层 let 与未声明赋值并存的 Legado 脚本（Rhino 非严格语义，七猫 ruleToc 形态）。
/// 旧实现先 eval 再失败重试，但首次执行已把 let 写入全局词法环境，
/// 同一 Context 重试必因 redeclaration 失败。var 补全必须发生在首次 eval 之前。
#[test]
fn eval_js_handles_let_plus_undeclared_assignment() {
    let script = r#"
function pick() { return 2; }
let factor = pick();
chapters = [{ title: "c1" }, { title: "c2" }];
chapters.length * factor
"#;
    let result = eval_js(script, "", "").unwrap();
    assert_eq!(result, "4");
}

/// 回归：var 补全名单必须排除 let/const 已声明名，否则补出的 var 与 let 冲突导致整段解析失败。
#[test]
fn eval_js_does_not_redeclare_let_names() {
    let script = r#"
let device = "android";
flag = device + "-ok";
flag
"#;
    let result = eval_js(script, "", "").unwrap();
    assert_eq!(result, "android-ok");
}

#[test]
fn eval_js_declares_unicode_legacy_globals() {
    let script = r#"
搜索接口1 = 1;
搜索接口2 = 搜索接口1 + 2;
搜索接口2
"#;
    let result = eval_js(script, "", "").unwrap();
    assert_eq!(result, "3");
}

#[test]
fn eval_js_declares_legacy_for_loop_variables() {
    let script = r#"
total = 0;
for (i in [1, 2, 3]) {
  total = total + Number(i);
}
for (j = 0; j < 2; j++) {
  total = total + j;
}
total
"#;
    let result = eval_js(script, "", "").unwrap();
    assert_eq!(result, "4");
}

#[test]
fn eval_js_handles_login_url_globals_shadowed_by_function_locals() {
    let script = r#"
function update() {
  let $$$ = { z: 9 };
  return $$$.z;
}
original = { z: 3, ml: 0 };
try {
  $$$ = JSON.parse("");
} catch (e) {
  $$$ = original;
}
JSON.stringify({ z: $$$.z, ml: $$$.ml, local: update() })
"#;
    let result = eval_js(script, "", "").unwrap();
    assert_eq!(result, r#"{"z":3,"ml":0,"local":9}"#);
}

#[test]
fn eval_js_round_trips_utf8_result_binding() {
    let text = r#"<p>二愣子睁大着双眼</p>"#;

    let result = eval_js("result", text, "").unwrap();

    assert_eq!(result, text);
}

#[test]
fn java_hex_decode_to_string_decodes_utf8_payloads() {
    let payload = r#"{"data":{"content":"<p>二愣子睁大着双眼</p>"}}"#;
    let encoded = hex::encode(payload.as_bytes());

    let result = eval_js(
        r#"
let decoded = java.hexDecodeToString(result);
JSON.parse(decoded).data.content
"#,
        &encoded,
        "",
    )
    .unwrap();

    assert_eq!(result, "<p>二愣子睁大着双眼</p>");
    assert!(!result.contains("äº"));
}

async fn js_search() -> Json<serde_json::Value> {
    Json(json!({
        "list": [{
            "name": "JS Route B Book",
            "author": "Codex",
            "bookUrl": "/book/1",
            "intro": "A JS source fixture."
        }]
    }))
}

async fn js_book() -> Json<serde_json::Value> {
    Json(json!({
        "name": "JS Route B Book",
        "author": "Codex",
        "intro": "A JS source fixture.",
        "tocUrl": "/book/1/toc"
    }))
}

async fn js_toc() -> Json<serde_json::Value> {
    Json(json!({
        "chapters": [{
            "name": "第一章 JS 原生路线",
            "url": "/chapter/1"
        }]
    }))
}

async fn js_content() -> Json<serde_json::Value> {
    Json(json!({ "content": "这是 JS 书源的第一章正文。" }))
}

async fn echo_body_hex(body: Bytes) -> String {
    hex::encode(body.as_ref())
}

#[tokio::test(flavor = "multi_thread")]
async fn okhttp_shim_preserves_base64_decoded_binary_request_body() {
    let app = Router::new().route("/echo-body", post(echo_body_hex));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let url = format!("http://{addr}/echo-body");

    use base64::Engine as _;
    let payload = [0x00, 0x01, 0x02, 0x7f, 0x80, 0xff, b'A'];
    let encoded = base64::engine::general_purpose::STANDARD.encode(payload);
    let expected = hex::encode(payload);

    let script = format!(
        r#"
const imports = new JavaImporter(Packages.okhttp3);
let responseText = '';
with (imports) {{
  const bytes = java.base64DecodeToByteArray('{encoded}');
  const request = new Request.Builder()
    .url('{url}')
    .post(RequestBody.create(bytes, MediaType.parse('application/octet-stream')))
    .build();
  responseText = new OkHttpClient().newCall(request).execute().body().string();
}}
responseText
"#
    );
    let result = eval_js(&script, "", "").unwrap();

    assert_eq!(result, expected);
    server.abort();
}

#[tokio::test]
async fn js_source_runtime_runs_main_reader_chain() {
    let app = Router::new()
        .route("/api/search", get(js_search))
        .route("/book/1", get(js_book))
        .route("/book/1/toc", get(js_toc))
        .route("/chapter/1", get(js_content));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let base = format!("http://{}", addr);

    let temp = tempfile::tempdir().unwrap();
    let core = ReaderCore::new(ReaderCoreOptions::new(temp.path()))
        .await
        .unwrap();
    let source = format!(
        r#"// @name        JS Fixture
// @url         {base}
// @enabled     true

const BASE_URL = '{base}';

async function search(key, page) {{
  const resp = await legado.http.get(`${{BASE_URL}}/api/search?keyword=${{encodeURIComponent(key)}}&page=${{page}}`);
  const json = JSON.parse(resp);
  return json.list.map(book => ({{
    name: book.name,
    author: book.author,
    bookUrl: BASE_URL + book.bookUrl,
    intro: book.intro,
  }}));
}}

async function bookInfo(bookUrl) {{
  const resp = await legado.http.get(bookUrl);
  const json = JSON.parse(resp);
  return {{
    name: json.name,
    author: json.author,
    intro: json.intro,
    bookUrl,
    tocUrl: BASE_URL + json.tocUrl,
  }};
}}

async function chapterList(tocUrl) {{
  const resp = await legado.http.get(tocUrl);
  const json = JSON.parse(resp);
  return json.chapters.map(ch => ({{
    name: ch.name,
    url: BASE_URL + ch.url,
  }}));
}}

async function chapterContent(chapterUrl) {{
  const resp = await legado.http.get(chapterUrl);
  return JSON.parse(resp).content;
}}

function chapterParagraphCommentCounts(chapterUrl, context) {{
  return {{
    "0+0": 2,
    "1+1": {{ count: context.paragraphCount > 1 ? 1 : 0 }}
  }};
}}

function chapterParagraphComments(chapterUrl, rangeKey, query) {{
  return {{
    comments: [{{
      id: `comment-${{rangeKey}}`,
      nickname: "Codex",
      content: `range=${{rangeKey}}, page=${{query.page || 1}}`,
      likeCount: 3,
      liked: false,
      replyCount: 0
    }}],
    total: 1,
    hasMore: false
  }};
}}

function likeParagraphComment(chapterUrl, rangeKey, commentId, liked) {{
  return {{ ok: true, commentId, liked }};
}}

function replyParagraphComment(chapterUrl, rangeKey, commentId, content) {{
  return {{ ok: true, commentId, content }};
}}
"#
    );
    core.save_js_source("js-fixture.js", &source, None)
        .await
        .unwrap();

    assert_eq!(
        core.eval_source_capabilities("js-fixture.js", None)
            .await
            .unwrap(),
        "search,bookInfo,toc,chapterList,content,chapterContent,chapterParagraphCommentCounts,chapterParagraphComments,likeParagraphComment,replyParagraphComment"
    );

    let comment_counts = core
        .source_call_fn(
            "js-fixture.js",
            "chapterParagraphCommentCounts",
            vec![json!("chapter://1"), json!({ "paragraphCount": 2 })],
            None,
        )
        .await
        .unwrap();
    assert_eq!(comment_counts["0+0"], 2);
    assert_eq!(comment_counts["1+1"]["count"], 1);

    let comment_details = core
        .source_call_fn(
            "js-fixture.js",
            "chapterParagraphComments",
            vec![
                json!("chapter://1"),
                json!("0+0"),
                json!({ "page": 2, "pageSize": 20 }),
            ],
            None,
        )
        .await
        .unwrap();
    assert_eq!(comment_details["comments"][0]["id"], "comment-0+0");
    assert_eq!(
        comment_details["comments"][0]["content"],
        "range=0+0, page=2"
    );

    let books = core
        .search("js-fixture.js", "Route", 1, None)
        .await
        .unwrap();
    assert_eq!(books.len(), 1);
    assert_eq!(books[0].name, "JS Route B Book");

    let detail = core
        .book_info("js-fixture.js", &books[0].book_url, None)
        .await
        .unwrap();
    assert_eq!(detail.name, "JS Route B Book");

    let chapters = core
        .chapter_list("js-fixture.js", detail.toc_url.as_deref().unwrap(), None)
        .await
        .unwrap();
    assert_eq!(chapters[0].name, "第一章 JS 原生路线");

    let content = core
        .chapter_content("js-fixture.js", &chapters[0].url, None)
        .await
        .unwrap();
    assert!(content.contains("JS 书源"));

    server.abort();
}

struct JsEngineTimeoutRestore;

impl Drop for JsEngineTimeoutRestore {
    fn drop(&mut self) {
        set_js_engine_timeout_secs(0);
    }
}

fn set_js_engine_timeout_for_test(secs: u64) -> JsEngineTimeoutRestore {
    set_js_engine_timeout_secs(secs);
    JsEngineTimeoutRestore
}

async fn assert_runaway_future_cancelled<T>(
    future: impl Future<Output = Result<T, reader_core::ReaderCoreError>>,
    cancel_token: Arc<AtomicBool>,
) where
    T: std::fmt::Debug,
{
    tokio::pin!(future);

    tokio::select! {
        result = &mut future => {
            panic!("runaway JS future finished before cancellation: {result:?}");
        },
        _ = tokio::time::sleep(Duration::from_millis(50)) => {}
    }

    let cancel_started = Instant::now();
    cancel_token.store(true, Ordering::SeqCst);
    let timed = tokio::time::timeout(Duration::from_secs(2), &mut future).await;
    let result =
        timed.expect("runaway JS future should observe cancellation before the engine timeout");

    assert!(
        result.is_err(),
        "cancelled runaway JS future should fail, got {result:?}"
    );
    assert!(
        cancel_started.elapsed() < Duration::from_secs(1),
        "JS cancellation should be prompt, elapsed={:?}",
        cancel_started.elapsed()
    );
}

#[tokio::test]
async fn js_search_cancel_token_interrupts_runaway_source() {
    let _timeout = set_js_engine_timeout_for_test(3);
    let temp = tempfile::tempdir().unwrap();
    let core = ReaderCore::new(ReaderCoreOptions::new(temp.path()))
        .await
        .unwrap();
    core.save_js_source(
        "runaway-search.js",
        r#"// @name        Runaway Search
// @url         https://example.invalid
// @enabled     true

async function search() {
  while (true) {}
}
"#,
        None,
    )
    .await
    .unwrap();

    let cancel_token = Arc::new(AtomicBool::new(false));
    let search = core.search_with_cancel(
        "runaway-search.js",
        "anything",
        1,
        None,
        Some(cancel_token.clone()),
    );
    assert_runaway_future_cancelled(search, cancel_token).await;
}

#[tokio::test]
async fn js_chapter_list_cancel_token_interrupts_runaway_source() {
    let _timeout = set_js_engine_timeout_for_test(3);
    let temp = tempfile::tempdir().unwrap();
    let core = ReaderCore::new(ReaderCoreOptions::new(temp.path()))
        .await
        .unwrap();
    core.save_js_source(
        "runaway-toc.js",
        r#"// @name        Runaway Toc
// @url         https://example.invalid
// @enabled     true

async function chapterList() {
  while (true) {}
}
"#,
        None,
    )
    .await
    .unwrap();

    let cancel_token = Arc::new(AtomicBool::new(false));
    let chapters = core.chapter_list_with_cancel(
        "runaway-toc.js",
        "https://example.invalid/book",
        None,
        Some(cancel_token.clone()),
    );
    assert_runaway_future_cancelled(chapters, cancel_token).await;
}

#[tokio::test]
async fn js_chapter_content_cancel_token_interrupts_runaway_source() {
    let _timeout = set_js_engine_timeout_for_test(3);
    let temp = tempfile::tempdir().unwrap();
    let core = ReaderCore::new(ReaderCoreOptions::new(temp.path()))
        .await
        .unwrap();
    core.save_js_source(
        "runaway-content.js",
        r#"// @name        Runaway Content
// @url         https://example.invalid
// @enabled     true

async function chapterContent() {
  while (true) {}
}
"#,
        None,
    )
    .await
    .unwrap();

    let cancel_token = Arc::new(AtomicBool::new(false));
    let content = core.chapter_content_with_cancel(
        "runaway-content.js",
        "https://example.invalid/chapter",
        None,
        Some(cancel_token.clone()),
    );
    assert_runaway_future_cancelled(content, cancel_token).await;
}

#[tokio::test]
async fn external_js_source_dirs_are_persisted_and_scanned() {
    let temp = tempfile::tempdir().unwrap();
    let external = tempfile::tempdir().unwrap();
    let external_source = external.path().join("external-fixture.js");
    tokio::fs::write(
        &external_source,
        r#"// @name        External JS Fixture
// @url         https://example.invalid
// @enabled     true

async function search() {
  return [];
}
"#,
    )
    .await
    .unwrap();

    let core = ReaderCore::new(ReaderCoreOptions::new(temp.path()))
        .await
        .unwrap();
    core.add_source_dir(external.path().to_str().unwrap())
        .await
        .unwrap();

    let dirs = core.source_dirs().await.unwrap();
    assert!(dirs
        .iter()
        .any(|dir| dir == &external.path().to_string_lossy()));

    let sources = core.list_sources().await.unwrap();
    assert!(sources.iter().any(|source| {
        source.file_name == "external-fixture.js"
            && source.name == "External JS Fixture"
            && source.source_dir == external.path().to_string_lossy()
    }));

    let core = ReaderCore::new(ReaderCoreOptions::new(temp.path()))
        .await
        .unwrap();
    let sources = core.list_sources().await.unwrap();
    assert!(sources
        .iter()
        .any(|source| source.file_name == "external-fixture.js"));

    core.remove_source_dir(external.path().to_str().unwrap())
        .await
        .unwrap();
    let sources = core.list_sources().await.unwrap();
    assert!(!sources
        .iter()
        .any(|source| source.file_name == "external-fixture.js"));
}

#[tokio::test]
async fn js_source_text_cache_refreshes_after_external_file_change() {
    let temp = tempfile::tempdir().unwrap();
    let core = ReaderCore::new(ReaderCoreOptions::new(temp.path()))
        .await
        .unwrap();
    let file_name = "cached-search.js";
    let path = core.js_source_dir().join(file_name);

    tokio::fs::write(
        &path,
        r#"// @name        Cached Search Fixture
// @url         https://example.invalid/v1
// @enabled     true

async function search() {
  return [{ name: "Old cached result", author: "Cache", bookUrl: "old://book" }];
}
"#,
    )
    .await
    .unwrap();

    let sources = core.list_sources().await.unwrap();
    assert!(sources.iter().any(|source| source.file_name == file_name));

    tokio::fs::write(
        &path,
        r#"// @name        Cached Search Fixture Updated
// @url         https://example.invalid/v2
// @enabled     true

async function search() {
  return [{ name: "Fresh cached result after disk update", author: "Cache", bookUrl: "fresh://book" }];
}
"#,
    )
    .await
    .unwrap();

    let books = core.search(file_name, "cache", 1, None).await.unwrap();
    assert_eq!(books[0].name, "Fresh cached result after disk update");
    assert_eq!(books[0].book_url, "fresh://book");
}

#[test]
fn new_js_apis_work() {
    // md5Encode16: first 16 chars of md5
    let result = eval_js("java.md5Encode16(result)", "hello", "").unwrap();
    assert_eq!(result.len(), 16);
    assert_eq!(result, "5d41402abc4b2a76");

    // timeFormatUTC
    let result = eval_js("java.timeFormatUTC(1700000000000)", "", "").unwrap();
    assert!(!result.is_empty());
    assert!(result.contains("T"));

    // base64DecodeToByteArray
    use base64::Engine as _;
    let input = base64::engine::general_purpose::STANDARD.encode(b"hello");
    let result = eval_js(
        &format!("String(java.base64DecodeToByteArray('{}'))", input),
        "",
        "",
    )
    .unwrap();
    assert_eq!(result, "hello");

    // toast / longToast / log: should not panic
    assert!(eval_js("java.toast('test')", "", "").is_ok());
    assert!(eval_js("java.longToast('test')", "", "").is_ok());
    assert!(eval_js("java.log('test')", "", "").is_ok());
    assert!(eval_js("java.log(new Error('network error'))", "", "").is_ok());
    assert!(eval_js("legado.log({ stage: 'compat' })", "", "").is_ok());

    // cookie.getKey
    eval_js("cookie.setCookie('ck_test', 'val')", "", "").unwrap();
    let result = eval_js("cookie.getKey('ck_test')", "", "").unwrap();
    assert_eq!(result, "val");

    // cache.delete
    eval_js("cache.put('del_test', 'val')", "", "").unwrap();
    eval_js("cache.delete('del_test')", "", "").unwrap();
    let result = eval_js("cache.get('del_test')", "", "").unwrap();
    assert!(result.is_empty());
    eval_js("cache.putFromMemory('mem_test', 'mem_val')", "", "").unwrap();
    let result = eval_js("cache.getFromMemory('mem_test')", "", "").unwrap();
    assert_eq!(result, "mem_val");

    // source.putLoginInfo / source.getLoginInfoMap
    eval_js("source.putLoginInfo('li_key', 'li_val')", "", "").unwrap();
    let result = eval_js("source.getLoginInfoMap().get('li_key')", "", "").unwrap();
    assert_eq!(result, "li_val");
}

#[test]
fn legado_rule_js_context_exposes_source_metadata_and_login_url() {
    let result = with_js_source(
        Some("function fromLib() { return source.bookSourceName; }"),
        Some("loginReady = 7;"),
        Some("番茄小说"),
        None,
        None,
        None,
        None,
        None,
        || {
            eval_js(
                "eval(String(source.loginUrl)); `${fromLib()}:${loginReady}`",
                "",
                "https://reading.snssdk.com#mgz0326",
            )
        },
    )
    .unwrap();

    assert_eq!(result, "番茄小说:7");
}

#[test]
fn legado_runtime_key_value_and_json_helpers_match_rule_usage() {
    let result = eval_js("java.put('tab_type', 3); java.get('tab_type')", "", "").unwrap();
    assert_eq!(result, "3");

    let result = eval_js(
        "java.getString('$.data.name')",
        r#"{"data":{"name":"番茄"}}"#,
        "",
    )
    .unwrap();
    assert_eq!(result, "番茄");

    let result = eval_js(
        r#"
source.setVariable(JSON.stringify({ z: 3, ml: 0 }));
const data = JSON.parse(source.getVariable());
data.z + ':' + data.ml
"#,
        "",
        "",
    )
    .unwrap();
    assert_eq!(result, "3:0");
}

#[test]
fn legado_chapter_variables_are_scoped_by_chapter_url() {
    let first = with_js_source(
        Some(""),
        None,
        Some("番茄小说"),
        Some("https://reading.snssdk.com#mgz0326"),
        Some("data:book_id;base64,NzI3NjM4NDEzODY1Mzg2Mjk2Ng=="),
        Some("https://reading.snssdk.com/chapter/7287058552051794491"),
        Some("第一章"),
        Some(1),
        || {
            eval_js(
                r#"
chapter.putVariable('fqContent', JSON.stringify(['第一段', '第二段']));
chapter.getVariable('fqContent')
"#,
                "",
                "https://reading.snssdk.com/chapter/7287058552051794491",
            )
        },
    )
    .unwrap();
    assert_eq!(first, r#"["第一段","第二段"]"#);

    let second = with_js_source(
        Some(""),
        None,
        Some("番茄小说"),
        Some("https://reading.snssdk.com#mgz0326"),
        Some("data:book_id;base64,NzI3NjM4NDEzODY1Mzg2Mjk2Ng=="),
        Some("https://reading.snssdk.com/chapter/7287058552051794491"),
        Some("第一章"),
        Some(1),
        || {
            eval_js(
                "chapter.getVariable('fqContent')",
                "",
                "https://reading.snssdk.com/chapter/7287058552051794491",
            )
        },
    )
    .unwrap();
    assert_eq!(second, first);

    let other_chapter = with_js_source(
        Some(""),
        None,
        Some("番茄小说"),
        Some("https://reading.snssdk.com#mgz0326"),
        Some("data:book_id;base64,NzI3NjM4NDEzODY1Mzg2Mjk2Ng=="),
        Some("https://reading.snssdk.com/chapter/7287058552051794492"),
        Some("第二章"),
        Some(2),
        || {
            eval_js(
                "chapter.getVariable('fqContent')",
                "",
                "https://reading.snssdk.com/chapter/7287058552051794492",
            )
        },
    )
    .unwrap();
    assert!(other_chapter.is_empty());
}

#[test]
fn legado_android_and_ajaxall_shims_are_available_without_ui_side_effects() {
    let result = eval_js(
        r#"
const javaImport = new JavaImporter();
javaImport.importPackage(Packages.okhttp3);
with (javaImport) {
  brand = String(Packages.android.os.Build.BRAND);
}
[
  brand.length > 0,
  DigestUtil.md5Hex('hello').length,
  StrUtil.reverse('abc'),
  Base64.decode(Base64.encode('ok')),
  java.ajaxAll([]).length,
  java.refreshExplore()
].join('|')
"#,
        "",
        "",
    )
    .unwrap();

    assert_eq!(result, "true|32|cba|ok|0|false");
}

#[test]
fn java_importer_with_block_functions_remain_visible_like_rhino() {
    let result = eval_js(
        r#"
const javaImport = new JavaImporter(Packages.okhttp3);
with (javaImport) {
  function compatRequestMediaType() {
    return MediaType.parse('application/octet-stream').type;
  }
}
compatRequestMediaType()
"#,
        "",
        "",
    )
    .unwrap();

    assert_eq!(result, "application/octet-stream");
}
